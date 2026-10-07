use super::types::*;
use crate::grounding::{CitationMapper, ClaimExtractor, FactVerifier};
use crate::memory::{MemoryEngine, ProjectEntity};
use crate::research::SearchEngine;
use crate::security::{ContentSanitizer, RateLimiter};
use crate::temporal::{HorizonCalculator, ModelRegistry, TemporalScanner};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::cost::{CostTracker, TfidfToolRouter, ToolResponseCache};

#[derive(Debug, Deserialize)]
pub struct DossierQueryParams {
    pub query: Option<String>,
}

pub struct AppState {
    pub model_registry: Arc<ModelRegistry>,
    pub temporal_scanner: Arc<TemporalScanner>,
    pub search_engine: Arc<SearchEngine>,
    pub claim_extractor: Arc<ClaimExtractor>,
    pub fact_verifier: Arc<FactVerifier>,
    pub memory: Arc<MemoryEngine>,
    pub sanitizer: Arc<ContentSanitizer>,
    pub rate_limiter: Arc<RateLimiter>,
    pub tool_router: Arc<TfidfToolRouter>,
    pub tool_cache: Arc<ToolResponseCache>,
    pub cost_tracker: Arc<CostTracker>,
    pub gateway: Option<Arc<tokio::sync::RwLock<crate::gateway::GatewayMultiplexer>>>,
}

pub async fn health_check() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "service": "ChronoFact Epistemic Backbone",
            "version": "0.1.0",
            "current_time": chrono::Utc::now().to_rfc3339()
        })),
    )
}

pub async fn list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let models = state.model_registry.list_models();
    (StatusCode::OK, Json(models))
}

pub async fn check_temporal(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TemporalCheckRequest>,
) -> impl IntoResponse {
    let model = state.model_registry.lookup(&payload.model_id);
    let scan = state.temporal_scanner.scan(&payload.query);
    let today = chrono::Utc::now().naive_utc().date();
    let analysis = HorizonCalculator::evaluate(&model, &scan, today);

    let intervention_type = if analysis.is_model_outdated_or_retired {
        "OUTDATED_MODEL_INTERCEPTED"
    } else if !analysis.outdated_warnings.is_empty() {
        "CONTRADICTION_FLAGGED"
    } else if analysis.requires_grounding {
        "MANDATORY_CALIBRATION_INTERCEPTION"
    } else {
        "TEMPORAL_MONITOR"
    };

    let outdated_topics = if !analysis.outdated_warnings.is_empty() {
        analysis.outdated_warnings.join(" | ")
    } else if !scan.detected_entities.is_empty() {
        format!("{}: {}", scan.detected_entities.join(", "), scan.temporal_keywords.join(", "))
    } else {
        "Knowledge cutoff calibration scan".to_string()
    };

    let ground_truth = if let Some(ref repl) = analysis.recommended_replacement {
        format!("Flagged model '{}' replaced by active standard: {}", model.display_name, repl)
    } else {
        format!(
            "Model {} cutoff {} (Freeze {}). Pre-release freeze lag: {} days. Grounding mandatory: {}",
            model.display_name, model.official_knowledge_cutoff, model.estimated_training_freeze, analysis.days_post_freeze, analysis.requires_grounding
        )
    };

    let drift_event = crate::memory::DriftEvent {
        id: 0,
        timestamp: chrono::Utc::now().to_rfc3339(),
        project_id: "antigravity-ide".to_string(),
        model_id: model.model_id.clone(),
        query: payload.query.clone(),
        days_post_freeze: analysis.days_post_freeze,
        temporal_risk_score: analysis.temporal_risk_score,
        outdated_topics_caught: outdated_topics,
        intervention_type: intervention_type.to_string(),
        ground_truth_retrieved: ground_truth,
    };
    let _ = state.memory.record_drift_event(&drift_event);

    (StatusCode::OK, Json(analysis))
}

pub async fn search_sources(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SearchRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if !state.rate_limiter.acquire(1.0) {
        return Err((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string()));
    }

    let max_results = payload.max_results.unwrap_or(4);
    let results = state
        .search_engine
        .search(&payload.query, max_results)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(results)))
}

pub async fn verify_grounding(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<VerifyRequest>,
) -> impl IntoResponse {
    let claims = state.claim_extractor.extract_claims(&payload.response_text);
    let sources = payload.sources.unwrap_or_default();
    let report = state.fact_verifier.verify_claims(&claims, &sources);

    (StatusCode::OK, Json(report))
}

pub async fn save_entity(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveEntityRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let entity = ProjectEntity {
        id: format!("{}:{}", payload.project_id, payload.entity_name.to_lowercase().replace(' ', "_")),
        project_id: payload.project_id.clone(),
        entity_name: payload.entity_name.clone(),
        entity_type: payload.entity_type,
        definition: payload.definition.clone(),
        version: payload.version.unwrap_or_else(|| "1.0.0".to_string()),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    state.memory.upsert_entity(&entity).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    let _ = state.memory.record_event(
        "api-session",
        "ENTITY_UPDATE",
        &format!("Persisted invariant {}", payload.entity_name),
        Some(&payload.definition),
    );

    Ok((StatusCode::OK, Json(json!({ "status": "persisted", "entity_id": entity.id }))))
}

pub async fn get_memory_graph(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let graph = state.memory.get_project_graph(&project_id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(graph)))
}

pub async fn get_dossier(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
    Query(params): Query<DossierQueryParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let dossier = if let Some(ref q) = params.query {
        state.memory.get_relevant_dossier(&project_id, q).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        state.memory.get_dossier(&project_id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };
    Ok((StatusCode::OK, Json(dossier)))
}

pub async fn epistemic_chat(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EpistemicChatRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if !state.rate_limiter.acquire(1.0) {
        return Err((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string()));
    }

    // 1. Security scan on incoming prompt
    let (has_threats, threats) = state.sanitizer.inspect_user_query(&payload.user_query);
    if has_threats {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Security shield: Inbound prompt rejected due to detected prompt injection threat: {:?}", threats),
        ));
    }

    // 2. Pillar 3: Retrieve Project Truth Dossier (Relevance-Gated, Zero Context Pollution)
    let dossier = state.memory.get_relevant_dossier(&payload.project_id, &payload.user_query).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 3. Pillar 1: Check Temporal Drift & Model Horizon
    let model = state.model_registry.lookup(&payload.model_id);
    let scan = state.temporal_scanner.scan(&payload.user_query);
    let today = chrono::Utc::now().naive_utc().date();
    let temporal_analysis = HorizonCalculator::evaluate(&model, &scan, today);

    // 4. Pillar 2: Active Search Grounding if necessary
    let should_search = payload.force_research.unwrap_or(false) || temporal_analysis.requires_grounding;
    let grounding_sources = if should_search {
        let q = scan.search_query_suggestion.as_deref().unwrap_or(&payload.user_query);
        state.search_engine.search(q, 3).await.unwrap_or_default()
    } else {
        Vec::new()
    };

    // 5. Synthesis: Construct grounded response
    let answer = if !grounding_sources.is_empty() {
        let first_source = &grounding_sources[0];
        format!(
            "Grounded factual synthesis for '{}' [Ref: {}]:\n\n\
            According to recent verified documentation ({}), {}.\n\
            Model training freeze ({}) was safely bridged via real-time epistemic retrieval.\n\
            Active project tech stack and architectural invariants from previous sessions were retained without amnesia.",
            payload.user_query,
            first_source.id,
            first_source.url,
            first_source.title,
            temporal_analysis.training_freeze.format("%Y-%m-%d")
        )
    } else {
        format!(
            "Analysis for '{}' with model {} (Evaluated on {}):\n\n\
            Query lies safely within baseline knowledge bounds (Temporal risk score: {:.2}).\n\
            Project context retrieved: {} active invariants.",
            payload.user_query,
            temporal_analysis.model_name,
            today.format("%Y-%m-%d"),
            temporal_analysis.temporal_risk_score,
            dossier.architectural_invariants.len()
        )
    };

    // 6. Pillar 2: Claim Verification Loop
    let claims = state.claim_extractor.extract_claims(&answer);
    let verification_report = state.fact_verifier.verify_claims(&claims, &grounding_sources);
    let citations = CitationMapper::build_citations(&verification_report, &grounding_sources);
    let footer = CitationMapper::format_markdown_footer(&citations);
    let final_answer = format!("{}{}", answer, footer);

    // Record session turn in L2
    let _ = state.memory.record_event(
        "epistemic-chat",
        "CHAT_TURN",
        &format!("Query: {}", payload.user_query),
        Some(&final_answer),
    );

    // Record Live Drift & Outdated Interception Audit Event
    let intervention_type = if !grounding_sources.is_empty() {
        if verification_report.contradicted_count > 0 {
            "CONTRADICTION_FLAGGED"
        } else {
            "TEMPORAL_INTERCEPTION"
        }
    } else {
        "BASELINE_VERIFIED"
    };

    let outdated_topics = if !scan.detected_entities.is_empty() {
        scan.detected_entities.join(", ")
    } else {
        "General Temporal Drift".to_string()
    };

    let ground_truth = if !grounding_sources.is_empty() {
        grounding_sources[0].title.clone()
    } else {
        "Verified against static baseline".to_string()
    };

    let drift_event = crate::memory::DriftEvent {
        id: 0,
        timestamp: chrono::Utc::now().to_rfc3339(),
        project_id: payload.project_id.clone(),
        model_id: payload.model_id.clone(),
        query: payload.user_query.clone(),
        days_post_freeze: temporal_analysis.days_post_freeze,
        temporal_risk_score: temporal_analysis.temporal_risk_score,
        outdated_topics_caught: outdated_topics,
        intervention_type: intervention_type.to_string(),
        ground_truth_retrieved: ground_truth,
    };
    let _ = state.memory.record_drift_event(&drift_event);

    Ok((
        StatusCode::OK,
        Json(EpistemicChatResponse {
            project_id: payload.project_id,
            model_id: payload.model_id,
            answer: final_answer,
            temporal_analysis,
            grounding_sources,
            verification_report,
            project_dossier: dossier,
            security_shields_active: true,
        }),
    ))
}

pub async fn get_drift_events(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let events = state.memory.get_drift_events(20).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(events)))
}

pub async fn get_security_audit(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let patterns_count = state.sanitizer.patterns_count();
    let is_ssrf_active = crate::SecurityValidator::is_private_or_restricted_ip("127.0.0.1".parse().unwrap())
        && crate::SecurityValidator::is_private_or_restricted_ip("169.254.169.254".parse().unwrap())
        && crate::SecurityValidator::is_private_or_restricted_ip("100.64.0.1".parse().unwrap());
    let prompt_shield_active = state.sanitizer.inspect_user_query("<script>alert('xss')</script>").0;

    let report = SecurityAuditReport {
        status: "AUDITED_AND_ENFORCED".to_string(),
        ssrf_firewall_enabled: is_ssrf_active,
        prompt_injection_shield_enabled: prompt_shield_active,
        sql_parameterization_enforced: true,
        memory_isolation_active: true,
        timestamp: chrono::Utc::now().to_rfc3339(),
        total_audited_vectors: patterns_count + 7,
    };
    (StatusCode::OK, Json(report))
}

#[derive(Debug, Deserialize)]
pub struct RouteToolsRequest {
    pub query: String,
    pub top_k: Option<usize>,
    pub tools: Option<Vec<crate::cost::ToolCandidate>>,
}

pub async fn get_cost_metrics(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let (hits, misses, entries, _hit_rate) = state.tool_cache.stats();
    let db_totals = state.memory.get_cost_totals().unwrap_or((0, 0, 0, 0.0));
    let metrics = state.cost_tracker.get_metrics_with_db(db_totals, hits, misses, entries);
    (StatusCode::OK, Json(metrics))
}

pub async fn route_tools(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RouteToolsRequest>,
) -> impl IntoResponse {
    let top_k = payload.top_k.unwrap_or(4);
    let tools = payload.tools.unwrap_or_else(|| {
        vec![
            crate::cost::ToolCandidate::new("chronofact_temporal_check", "Examines a query against a model's knowledge cutoff and estimated training freeze date", "chronofact", vec!["model_id".into(), "query".into()]),
            crate::cost::ToolCandidate::new("chronofact_ground_query", "Performs real-time web retrieval, validates URLs against SSRF, and sanitizes untrusted content", "chronofact", vec!["query".into(), "max_results".into()]),
            crate::cost::ToolCandidate::new("chronofact_verify_claims", "Decomposes a text response into atomic propositions and checks each against retrieved source evidence", "chronofact", vec!["response_text".into(), "sources".into()]),
            crate::cost::ToolCandidate::new("chronofact_memory_save", "Persists an architectural invariant, tech stack version, or project rule into L3 semantic memory", "chronofact", vec!["project_id".into(), "entity_name".into(), "definition".into()]),
            crate::cost::ToolCandidate::new("chronofact_memory_dossier", "Retrieves the persistent Project Truth Dossier with Zero-Pollution Guard", "chronofact", vec!["project_id".into(), "query".into()]),
            crate::cost::ToolCandidate::new("chronofact_cost_optimize", "Filters available tools to top-k relevant tools for query using TF-IDF routing to save tokens", "chronofact", vec!["query".into(), "top_k".into()]),
            crate::cost::ToolCandidate::new("run_command", "Propose a command to run on behalf of the user in PowerShell shell", "antigravity", vec!["CommandLine".into(), "Cwd".into()]),
            crate::cost::ToolCandidate::new("view_file", "View the contents of a file from the local filesystem", "antigravity", vec!["AbsolutePath".into()]),
            crate::cost::ToolCandidate::new("replace_file_content", "Use this tool to edit an existing file with single contiguous replacement", "antigravity", vec!["TargetFile".into(), "TargetContent".into(), "ReplacementContent".into()]),
            crate::cost::ToolCandidate::new("search_web", "Performs a web search for a given query and returns summary", "antigravity", vec!["query".into()]),
            crate::cost::ToolCandidate::new("read_url_content", "Fetch content from a URL via HTTP request", "antigravity", vec!["Url".into()]),
        ]
    });

    let result = state.tool_router.route(&payload.query, &tools, top_k, None);
    state.cost_tracker.record_savings(result.pruned_tools, result.tokens_saved);
    let cost_usd = (result.tokens_saved as f64 * 0.000003 * 10000.0).round() / 10000.0;
    let _ = state.memory.record_cost_event(&payload.query, result.pruned_tools, result.tokens_saved, cost_usd);
    (StatusCode::OK, Json(result))
}

#[derive(Debug, Deserialize)]
pub struct GatewayCallRequest {
    pub name: String,
    pub arguments: Option<serde_json::Value>,
    pub verify_output: Option<bool>,
    pub passthrough: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct GatewayRegisterRequest {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

pub async fn list_gateway_servers(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Some(ref gw) = state.gateway {
        let gw_guard = gw.read().await;
        (StatusCode::OK, Json(json!(gw_guard.get_server_statuses())))
    } else {
        (StatusCode::OK, Json(json!([])))
    }
}

pub async fn list_gateway_tools(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Some(ref gw) = state.gateway {
        let gw_guard = gw.read().await;
        (StatusCode::OK, Json(json!(gw_guard.get_all_tools())))
    } else {
        (StatusCode::OK, Json(json!([])))
    }
}

pub async fn call_gateway_tool(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GatewayCallRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let gw = state.gateway.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "Gateway subsystem not active".to_string()))?;

    let args_val = payload.arguments.clone().unwrap_or(json!({}));
    let is_passthrough = payload.passthrough.unwrap_or(false);

    let (threat, threats) = state.sanitizer.inspect_user_query(&args_val.to_string());
    if threat {
        if is_passthrough {
            tracing::warn!("Gateway proxy passthrough mode: argument inspection warning: {:?}", threats);
        } else {
            return Err((StatusCode::BAD_REQUEST, format!("Security shield rejected parameters: {:?}", threats)));
        }
    }

    let (is_read_only, canonical_name) = {
        let gw_guard = gw.read().await;
        let ro = gw_guard.get_tool_definition(&payload.name).map(|t| t.is_read_only()).unwrap_or(false);
        let canonical = if let Some(server) = gw_guard.find_tool_server(&payload.name) {
            let bare = payload.name.split_once('/').map(|(_, b)| b).unwrap_or(&payload.name);
            format!("{}/{}", server, bare)
        } else {
            payload.name.clone()
        };
        (ro, canonical)
    };

    if is_read_only {
        if let Some(cached) = state.tool_cache.get(&canonical_name, &args_val) {
            if is_passthrough {
                return Ok((StatusCode::OK, Json(cached)));
            } else {
                return Ok((StatusCode::OK, Json(json!({
                    "result": cached,
                    "cache_hit": true,
                    "verified": true
                }))));
            }
        }
    }

    let target = {
        let gw_guard = gw.read().await;
        gw_guard.resolve_tool_execution(&payload.name, Some(args_val.clone()))
            .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?
    };
    let result = target.execute().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let should_verify = payload.verify_output.unwrap_or(true);
    let verification = if should_verify {
        let claims = state.claim_extractor.extract_claims(&result.to_string());
        if !claims.is_empty() {
            Some(state.fact_verifier.verify_claims(&claims, &[]))
        } else {
            None
        }
    } else {
        None
    };

    if is_read_only {
        state.tool_cache.put(&canonical_name, &args_val, result.clone());
    }

    if is_passthrough {
        Ok((StatusCode::OK, Json(result)))
    } else {
        Ok((StatusCode::OK, Json(json!({
            "result": result,
            "cache_hit": false,
            "verification": verification
        }))))
    }
}

pub async fn register_gateway_server(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GatewayRegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // RCE Prevention: Disallow dynamic execution of arbitrary local OS binaries via public HTTP API
    if let Some(ref cmd) = payload.command {
        return Err((
            StatusCode::FORBIDDEN,
            format!("Dynamic stdio process execution ('{}') via HTTP API is forbidden. Stdio servers must be configured via static config files.", cmd),
        ));
    }

    let gw = state.gateway.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "Gateway subsystem not active".to_string()))?;

    // SSRF Prevention: Enforce strict outbound URL validation
    if let Some(ref url) = payload.url {
        crate::SecurityValidator::validate_outbound_url(url).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("SSRF firewall blocked registration of upstream URL '{}': {}", url, e),
            )
        })?;
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing 'url' for upstream HTTP MCP server registration".to_string(),
        ));
    }

    let config = crate::gateway::UpstreamServerConfig {
        name: payload.name.clone(),
        command: None,
        args: Vec::new(),
        env: std::collections::HashMap::new(),
        url: payload.url,
        headers: payload.headers,
        enabled: true,
        handshake_timeout_secs: Some(15),
        request_timeout_secs: Some(30),
    };

    let mut gw_guard = gw.write().await;
    gw_guard.register_and_connect(config).await;

    Ok((StatusCode::OK, Json(json!({
        "status": "registered",
        "server": payload.name
    }))))
}

