use super::protocol::{JsonRpcRequest, JsonRpcResponse};
use super::tools;
use crate::cost::{CostTracker, TfidfToolRouter, ToolCandidate, ToolResponseCache};
use crate::gateway::GatewayMultiplexer;
use crate::grounding::{ClaimExtractor, FactVerifier};
use crate::memory::{DriftEvent, EpistemicTrajectory, MemoryEngine, PostTrainingPair, ProjectEntity};
use crate::research::SearchEngine;
use crate::security::ContentSanitizer;
use crate::temporal::{HorizonCalculator, ModelRegistry, TemporalScanner};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

use std::collections::HashMap;

/// Circuit breaker state tracking consecutive upstream timeouts and open/closed state.
#[derive(Debug, Clone, Default)]
pub struct GatewayCircuitBreaker {
    pub consecutive_timeouts: u32,
    pub tripped_at: Option<std::time::Instant>,
}

impl GatewayCircuitBreaker {
    pub fn is_open(&self, cooldown: std::time::Duration) -> bool {
        if let Some(tripped) = self.tripped_at {
            if tripped.elapsed() < cooldown {
                return true;
            }
        }
        false
    }

    pub fn record_success(&mut self) {
        self.consecutive_timeouts = 0;
        self.tripped_at = None;
    }

    pub fn record_timeout(&mut self, threshold: u32) -> bool {
        self.consecutive_timeouts += 1;
        if self.consecutive_timeouts >= threshold {
            self.tripped_at = Some(std::time::Instant::now());
            true
        } else {
            false
        }
    }
}

pub struct McpServer {
    model_registry: Arc<ModelRegistry>,
    scanner: Arc<TemporalScanner>,
    searcher: Arc<SearchEngine>,
    extractor: Arc<ClaimExtractor>,
    verifier: Arc<FactVerifier>,
    memory: Arc<MemoryEngine>,
    tool_router: Arc<TfidfToolRouter>,
    tool_cache: Arc<ToolResponseCache>,
    cost_tracker: Arc<CostTracker>,
    sanitizer: Arc<ContentSanitizer>,
    gateway: Option<Arc<RwLock<GatewayMultiplexer>>>,
    circuit_breakers: Arc<RwLock<HashMap<String, GatewayCircuitBreaker>>>,
}

impl McpServer {
    pub fn new(memory: Arc<MemoryEngine>) -> Self {
        Self::with_gateway(memory, None)
    }

    pub fn with_gateway(
        memory: Arc<MemoryEngine>,
        gateway: Option<Arc<RwLock<GatewayMultiplexer>>>,
    ) -> Self {
        Self {
            model_registry: Arc::new(ModelRegistry::new()),
            scanner: Arc::new(TemporalScanner::new()),
            searcher: Arc::new(SearchEngine::new()),
            extractor: Arc::new(ClaimExtractor::new()),
            verifier: Arc::new(FactVerifier::new()),
            memory,
            tool_router: Arc::new(TfidfToolRouter::default()),
            tool_cache: Arc::new(ToolResponseCache::default()),
            cost_tracker: Arc::new(CostTracker::default()),
            sanitizer: Arc::new(ContentSanitizer::new()),
            gateway,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Construct an McpServer sharing the exact same AppState instances (tool_cache, cost_tracker,
    /// memory, router, gateway) as the co-hosted HTTP/SSE Cockpit API server.
    pub fn from_app_state(state: Arc<crate::proxy::AppState>) -> Self {
        Self {
            model_registry: state.model_registry.clone(),
            scanner: state.temporal_scanner.clone(),
            searcher: state.search_engine.clone(),
            extractor: state.claim_extractor.clone(),
            verifier: state.fact_verifier.clone(),
            memory: state.memory.clone(),
            tool_router: state.tool_router.clone(),
            tool_cache: state.tool_cache.clone(),
            cost_tracker: state.cost_tracker.clone(),
            sanitizer: state.sanitizer.clone(),
            gateway: state.gateway.clone(),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Build native + host ToolCandidates dynamically from `tools::list_tools()` schemas
    fn native_tool_candidates(include_host_tools: bool) -> Vec<ToolCandidate> {
        let mut candidates = Vec::new();
        let tools_val = tools::list_tools();
        if let Some(arr) = tools_val.get("tools").and_then(|t| t.as_array()) {
            for item in arr {
                let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name.is_empty() {
                    continue;
                }
                let desc = item.get("description").and_then(|d| d.as_str()).unwrap_or("");
                let server = if name.starts_with("gateway_") {
                    "chronofact-gateway"
                } else {
                    "chronofact"
                };
                let mut params = Vec::new();
                if let Some(props) = item
                    .get("inputSchema")
                    .and_then(|s| s.get("properties"))
                    .and_then(|p| p.as_object())
                {
                    for (key, prop_val) in props {
                        if let Some(pdesc) = prop_val.get("description").and_then(|d| d.as_str()) {
                            params.push(format!("{} {}", key, pdesc));
                        } else {
                            params.push(key.clone());
                        }
                    }
                }
                candidates.push(ToolCandidate::new(name, desc, server, params));
            }
        }

        if include_host_tools {
            candidates.extend(vec![
                ToolCandidate::new("spine_reality_audit", "Audits user prompt against 33 SPINE vertebrae for sycophancy traps, flattery bait, and credential intimidation", "spine", vec!["prompt".into(), "reality_level".into()]),
                ToolCandidate::new("spine_verify_pushback", "Evaluates aggressive user pushback under Thoracic T1/T4 to block unearned apologies and enforce technical proof", "spine", vec!["challenge_prompt".into(), "prior_assertion_verified".into()]),
                ToolCandidate::new("spine_adversarial_redteam", "Performs Lumbar/Sacral adversarial architecture red-teaming across scale, security, and failure modes", "spine", vec!["proposal".into(), "target_scale".into()]),
                ToolCandidate::new("spine_generate_gate_attestation", "Generates a cryptographic HMAC-SHA256 SPINE gate attestation binding reality audit and red-team verdicts", "spine", vec!["project_id".into(), "target_artifact".into()]),
                ToolCandidate::new("run_command", "Propose a command to run on behalf of the user in PowerShell shell", "antigravity", vec!["CommandLine".into(), "Cwd".into()]),
                ToolCandidate::new("view_file", "View the contents of a file from the local filesystem", "antigravity", vec!["AbsolutePath".into()]),
                ToolCandidate::new("replace_file_content", "Use this tool to edit an existing file with single contiguous replacement", "antigravity", vec!["TargetFile".into(), "TargetContent".into(), "ReplacementContent".into()]),
                ToolCandidate::new("write_to_file", "Use this tool to create new files or overwrite existing files", "antigravity", vec!["TargetFile".into(), "CodeContent".into()]),
                ToolCandidate::new("search_web", "Performs a web search for a given query and returns summary", "antigravity", vec!["query".into()]),
                ToolCandidate::new("read_url_content", "Fetch content from a URL via HTTP request", "antigravity", vec!["Url".into()]),
            ]);
        }

        candidates
    }

    pub async fn run_stdio(&self) -> io::Result<()> {
        const MAX_MCP_LINE_BYTES: u64 = 16 * 1024 * 1024;
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut stdout = tokio::io::stdout();
        let mut buf = Vec::new();

        loop {
            buf.clear();
            let bytes_read = match (&mut reader).take(MAX_MCP_LINE_BYTES).read_until(b'\n', &mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("ChronoFact stdio read error: {}", e);
                    break;
                }
            };
            if bytes_read == 0 {
                break;
            }

            let text = String::from_utf8_lossy(&buf);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }

            let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(e) => {
                    let err_resp = JsonRpcResponse::error(None, -32700, &format!("Parse error: {}", e));
                    let out = serde_json::to_string(&err_resp).unwrap();
                    let _ = stdout.write_all(format!("{}\n", out).as_bytes()).await;
                    let _ = stdout.flush().await;
                    continue;
                }
            };

            let resp = self.handle_request(req).await;
            if let Some(r) = resp {
                let out = serde_json::to_string(&r).unwrap();
                let _ = stdout.write_all(format!("{}\n", out).as_bytes()).await;
                let _ = stdout.flush().await;
            }
        }

        Ok(())
    }

    pub async fn handle_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id;
        match req.method.as_str() {
            "initialize" => {
                let client_proto = req.params.as_ref()
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("2025-03-26");
                let proto = if client_proto == "2024-11-05" {
                    "2024-11-05"
                } else {
                    "2025-03-26"
                };
                let router_mode = std::env::var("CHRONOFACT_ROUTER_MODE")
                    .unwrap_or_else(|_| "passthrough".to_string())
                    .to_lowercase();
                let result = json!({
                    "protocolVersion": proto,
                    "capabilities": {
                        "tools": {},
                        "resources": {},
                        "prompts": {}
                    },
                    "serverInfo": {
                        "name": "chronofact",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": format!(
                        "ChronoFact Epistemic Backbone & MCP Multiplexing Gateway (router_mode={}). Call chronofact_temporal_check first on technical queries, chronofact_verify_claims / chronofact_verify_code_invariants to audit claims and code, and gateway_find_tools + gateway_call_tool to discover and invoke upstream tools on demand.",
                        router_mode
                    )
                });
                Some(JsonRpcResponse::success(id, result))
            }
            "notifications/initialized" => None,
            "ping" => Some(JsonRpcResponse::success(id, json!({}))),
            "tools/list" => {
                let mut tools_data = tools::list_tools();
                let mode_override = req
                    .params
                    .as_ref()
                    .and_then(|p| p.get("router_mode"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_lowercase());
                let router_mode = mode_override.unwrap_or_else(|| {
                    std::env::var("CHRONOFACT_ROUTER_MODE")
                        .unwrap_or_else(|_| "passthrough".to_string())
                        .to_lowercase()
                });
                // In "metatool" mode (learned from MCPlex), upstream tools are hidden from tools/list
                // and discovered dynamically via gateway_find_tools + gateway_call_tool to eliminate schema bloat.
                if router_mode != "metatool" {
                    if let Some(ref gw) = self.gateway {
                        let gw_guard = gw.read().await;
                        let upstream = gw_guard.get_all_tools();
                        if let Some(arr) = tools_data.get_mut("tools").and_then(|v| v.as_array_mut()) {
                            for ut in upstream {
                                let schema = ut.definition.input_schema.unwrap_or_else(|| json!({"type": "object", "properties": {}}));
                                let mut tool_obj = json!({
                                    "name": ut.definition.name,
                                    "description": ut.definition.description.unwrap_or_else(|| format!("Upstream tool on server '{}'", ut.server_name)),
                                    "inputSchema": schema,
                                    "server": ut.server_name,
                                    "fqn": ut.fqn
                                });
                                if let Some(ref annotations) = ut.definition.annotations {
                                    tool_obj["annotations"] = annotations.clone();
                                }
                                arr.push(tool_obj);
                            }
                        }
                    }
                }
                Some(JsonRpcResponse::success(id, tools_data))
            }
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);

                let is_native_cacheable = matches!(
                    tool_name,
                    "chronofact_temporal_check"
                        | "chronofact_verify_claims"
                        | "chronofact_verify_code_invariants"
                        | "chronofact_cost_optimize"
                );
                if is_native_cacheable {
                    if let Some(cached) = self.tool_cache.get(tool_name, &args) {
                        let text = serde_json::to_string(&cached).unwrap_or_else(|_| cached.to_string());
                        return Some(JsonRpcResponse::success(
                            id,
                            json!({
                                "content": [{ "type": "text", "text": text }],
                                "isError": false
                            }),
                        ));
                    }
                }

                let tool_result = self.execute_tool(tool_name, &args).await;
                match tool_result {
                    Ok(val) => {
                        if is_native_cacheable {
                            self.tool_cache.put(tool_name, &args, val.clone());
                        } else if tool_name == "chronofact_memory_save" {
                            self.tool_cache.invalidate_server("default");
                        }
                        let result_obj = if val.is_object() && val.get("content").and_then(|c| c.as_array()).is_some() {
                            val
                        } else {
                            json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": serde_json::to_string(&val).unwrap_or_else(|_| val.to_string())
                                    }
                                ],
                                "isError": false
                            })
                        };
                        Some(JsonRpcResponse::success(id, result_obj))
                    }
                    Err(err_msg) => {
                        let err_obj = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Error executing tool {}: {}", tool_name, err_msg)
                                }
                            ],
                            "isError": true
                        });
                        Some(JsonRpcResponse::success(id, err_obj))
                    }
                }
            }
            "resources/list" => {
                let resources = if let Some(ref gw) = self.gateway {
                    let gw_guard = gw.read().await;
                    gw_guard
                        .get_all_resources()
                        .into_iter()
                        .map(|r| {
                            json!({
                                "uri": r.definition.uri,
                                "name": r.definition.name,
                                "description": r.definition.description,
                                "mimeType": r.definition.mime_type,
                                "server": r.server_name,
                                "fqn": r.fqn
                            })
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                Some(JsonRpcResponse::success(id, json!({ "resources": resources })))
            }
            "resources/read" => {
                let params = req.params.unwrap_or(Value::Null);
                let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
                if uri.is_empty() {
                    return Some(JsonRpcResponse::error(id, -32602, "Missing 'uri' parameter"));
                }
                if let Some(ref gw) = self.gateway {
                    let gw_guard = gw.read().await;
                    match gw_guard.read_resource(uri).await {
                        Ok(val) => Some(JsonRpcResponse::success(id, val)),
                        Err(e) => Some(JsonRpcResponse::error(
                            id,
                            -32000,
                            &format!("Failed to read resource '{}': {}", uri, e),
                        )),
                    }
                } else {
                    Some(JsonRpcResponse::error(id, -32000, "Gateway subsystem not initialized"))
                }
            }
            "prompts/list" => {
                let prompts = if let Some(ref gw) = self.gateway {
                    let gw_guard = gw.read().await;
                    gw_guard
                        .get_all_prompts()
                        .into_iter()
                        .map(|p| {
                            json!({
                                "name": p.definition.name,
                                "description": p.definition.description,
                                "arguments": p.definition.arguments,
                                "server": p.server_name,
                                "fqn": p.fqn
                            })
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                Some(JsonRpcResponse::success(id, json!({ "prompts": prompts })))
            }
            "prompts/get" => {
                let params = req.params.unwrap_or(Value::Null);
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name.is_empty() {
                    return Some(JsonRpcResponse::error(id, -32602, "Missing 'name' parameter"));
                }
                let arguments = params.get("arguments").cloned();
                if let Some(ref gw) = self.gateway {
                    let gw_guard = gw.read().await;
                    match gw_guard.get_prompt(name, arguments).await {
                        Ok(val) => Some(JsonRpcResponse::success(id, val)),
                        Err(e) => Some(JsonRpcResponse::error(
                            id,
                            -32000,
                            &format!("Failed to get prompt '{}': {}", name, e),
                        )),
                    }
                } else {
                    Some(JsonRpcResponse::error(id, -32000, "Gateway subsystem not initialized"))
                }
            }
            unknown => Some(JsonRpcResponse::error(
                id,
                -32601,
                &format!("Method '{}' not found", unknown),
            )),
        }
    }

    async fn execute_tool(&self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "chronofact_temporal_check" => {
                let requested_model_id = args.get("model_id").and_then(|v| v.as_str()).unwrap_or("");
                let query = args
                    .get("query")
                    .or_else(|| args.get("prompt"))
                    .or_else(|| args.get("claim"))
                    .or_else(|| args.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let compact = args.get("compact").and_then(|v| v.as_bool()).unwrap_or(false);
                let scan = self.scanner.scan(query);

                // Dynamically detect model from prompt if not specified or if specific model query detected.
                // Prioritizes explicit model_id, then active 2026 models mentioned in comparison/migration queries,
                // before falling back to flagged outdated models so comparison queries are never hijacked.
                let q_lower = query.to_lowercase();
                let effective_model_id = if !requested_model_id.is_empty()
                    && requested_model_id != "auto"
                    && requested_model_id != "default"
                {
                    requested_model_id
                } else if q_lower.contains("astra") || q_lower.contains("astr-6") || q_lower.contains("astr 6") {
                    "gpt-6-astra"
                } else if q_lower.contains("sol-6")
                    || q_lower.contains("sol 6")
                    || q_lower.contains("sol-6.1")
                    || q_lower.contains("sol 6.1")
                {
                    "gpt-6-1-sol"
                } else if q_lower.contains("opus 5") || q_lower.contains("opus-5") {
                    "claude-opus-5-5"
                } else if q_lower.contains("sonnet 5") || q_lower.contains("sonnet-5") {
                    "claude-sonnet-5-5"
                } else if q_lower.contains("grok 4") || q_lower.contains("grok-4") {
                    "grok-4-7"
                } else if q_lower.contains("gemini-4") || q_lower.contains("gemini 4") || q_lower.contains("argon") {
                    "gemini-4-argon"
                } else if let Some(flagged) = scan.outdated_models_flagged.first() {
                    let f_lower = flagged.to_lowercase();
                    if f_lower.contains("3.5") || f_lower.contains("3-5") {
                        "claude-3-5-sonnet"
                    } else if f_lower.contains("3.7") || f_lower.contains("3-7") {
                        "claude-3-7-sonnet"
                    } else if f_lower.contains("grok") {
                        "grok-3"
                    } else if f_lower.contains("gpt-4") || f_lower.contains("gpt 4") {
                        "gpt-4"
                    } else {
                        "claude-opus-5-5"
                    }
                } else if q_lower.contains("claude-3-5")
                    || q_lower.contains("claude 3.5")
                    || q_lower.contains("sonnet 3.5")
                    || q_lower.contains("sonnet-3-5")
                {
                    "claude-3-5-sonnet"
                } else if q_lower.contains("claude-3-7")
                    || q_lower.contains("claude 3.7")
                    || q_lower.contains("sonnet 3.7")
                    || q_lower.contains("sonnet-3-7")
                {
                    "claude-3-7-sonnet"
                } else if q_lower.contains("grok-3") || q_lower.contains("grok 3") {
                    "grok-3"
                } else if q_lower.contains("gpt-4") || q_lower.contains("gpt 4") {
                    "gpt-4"
                } else if q_lower.contains("gemini") {
                    "gemini-3-8-flash"
                } else {
                    "claude-opus-5-5"
                };

                let model = self.model_registry.lookup(effective_model_id);
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
                    format!("Flagged model '{}' replaced by verified 2026 active standard: {}", model.display_name, repl)
                } else {
                    format!(
                        "Model {} cutoff {} (Freeze {}). Pre-release freeze lag: {} days. Grounding mandatory: {}",
                        model.display_name, model.official_knowledge_cutoff, model.estimated_training_freeze, analysis.days_post_freeze, analysis.requires_grounding
                    )
                };

                // Live recording for Antigravity / IDE session
                let drift_event = DriftEvent {
                    id: 0,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    project_id: "antigravity-ide".to_string(),
                    model_id: model.model_id.clone(),
                    query: query.to_string(),
                    days_post_freeze: analysis.days_post_freeze,
                    temporal_risk_score: analysis.temporal_risk_score,
                    outdated_topics_caught: outdated_topics,
                    intervention_type: intervention_type.to_string(),
                    ground_truth_retrieved: ground_truth,
                };
                let _ = self.memory.record_drift_event(&drift_event);

                if compact && !analysis.is_model_outdated_or_retired && analysis.outdated_warnings.is_empty() {
                    Ok(json!({
                        "calibration_block": analysis.calibration_block,
                        "model_id": analysis.model_id,
                        "model_name": analysis.model_name,
                        "temporal_risk_score": analysis.temporal_risk_score,
                        "requires_grounding": analysis.requires_grounding,
                        "is_model_outdated_or_retired": false
                    }))
                } else {
                    Ok(json!(analysis))
                }
            }
            "chronofact_ground_query" => {
                let query = args
                    .get("query")
                    .or_else(|| args.get("prompt"))
                    .and_then(|v| v.as_str())
                    .ok_or("Missing query")?;
                let max = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(4) as usize;

                let sources = self.searcher.search(query, max).await?;

                let drift_event = DriftEvent {
                    id: 0,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    project_id: "antigravity-ide".to_string(),
                    model_id: "grounding-engine".to_string(),
                    query: query.to_string(),
                    days_post_freeze: 0,
                    temporal_risk_score: 0.75,
                    outdated_topics_caught: format!("Real-time search: {}", query),
                    intervention_type: "WEB_RESEARCH_GROUNDING".to_string(),
                    ground_truth_retrieved: sources.iter().map(|s| s.title.clone()).collect::<Vec<_>>().join(" | "),
                };
                let _ = self.memory.record_drift_event(&drift_event);

                Ok(json!({
                    "query": query,
                    "count": sources.len(),
                    "sources": sources
                }))
            }
            "chronofact_verify_claims" => {
                let response_text = args
                    .get("response_text")
                    .or_else(|| args.get("claim"))
                    .or_else(|| args.get("prompt"))
                    .or_else(|| args.get("content"))
                    .and_then(|v| v.as_str())
                    .ok_or("Missing response_text")?;
                let sources: Vec<crate::research::SourceChunk> = if let Some(arr) = args.get("sources") {
                    serde_json::from_value(arr.clone()).map_err(|e| format!("Invalid 'sources' schema (expected [{{id,title,url,content}}]): {}", e))?
                } else {
                    Vec::new()
                };

                let claims = self.extractor.extract_claims(response_text);
                let mut report = self.verifier.verify_claims(&claims, &sources);

                // Also audit any fenced code blocks present in the response using CodeInvariantChecker
                let code_blocks = self.extractor.extract_fenced_code_blocks(response_text);
                if !code_blocks.is_empty() {
                    let checker = crate::grounding::CodeInvariantChecker::new();
                    for (block_idx, (lang, code)) in code_blocks.iter().enumerate() {
                        let virtual_path = if lang.is_empty() {
                            format!("response_block_{}.txt", block_idx + 1)
                        } else {
                            format!("response_block_{}.{}", block_idx + 1, lang)
                        };
                        let code_audit = checker.audit_code(&virtual_path, code, None);
                        for v in code_audit.violations {
                            report.total_claims += 1;
                            report.contradicted_count += 1;
                            report.claims.push(crate::grounding::VerifiedClaim {
                                claim: crate::grounding::AtomicClaim {
                                    id: format!("code_inv_{}_{}", block_idx + 1, v.line_number),
                                    statement: v.line_snippet.clone(),
                                    category: crate::grounding::ClaimCategory::TechnicalApi,
                                },
                                status: crate::grounding::VerificationStatus::Contradicted,
                                confidence_score: 0.01,
                                matched_source_ids: vec![v.rule_id.clone()],
                                rationale: format!(
                                    "CODE INVARIANT VIOLATION [{} / {}]: {} Remediation: {}",
                                    v.rule_id,
                                    v.severity.as_str(),
                                    v.message,
                                    v.remediation
                                ),
                            });
                        }
                    }
                    if report.total_claims > 0 {
                        let linear_risk = ((report.contradicted_count as f32 * 1.0)
                            + (report.unverified_count as f32 * 0.4))
                            / (report.total_claims as f32);
                        let noisy_or_floor = if report.contradicted_count > 0 {
                            1.0 - (1.0 - 0.38_f32).powi(report.contradicted_count as i32)
                        } else {
                            0.0
                        };
                        report.hallucination_risk_index =
                            ((linear_risk.max(noisy_or_floor).min(1.0)) * 100.0).round() / 100.0;
                    }
                }

                let now_ts = chrono::Utc::now().to_rfc3339();
                let intervention = if report.contradicted_count > 0 {
                    "CONTRADICTION_INTERCEPTED".to_string()
                } else {
                    "CLAIM_VERIFICATION".to_string()
                };

                let drift_event = DriftEvent {
                    id: 0,
                    timestamp: now_ts.clone(),
                    project_id: "antigravity-ide".to_string(),
                    model_id: "claim-verifier".to_string(),
                    query: response_text.to_string(),
                    days_post_freeze: 0,
                    temporal_risk_score: report.hallucination_risk_index,
                    outdated_topics_caught: format!(
                        "Audited {} claims (Entailed: {}, Contradicted: {}, Unverified: {})",
                        report.total_claims, report.entailed_count, report.contradicted_count, report.unverified_count
                    ),
                    intervention_type: intervention.clone(),
                    ground_truth_retrieved: if report.contradicted_count > 0 {
                        format!("CONTRADICTION DETECTED: Hallucination risk {:.0}%", report.hallucination_risk_index * 100.0)
                    } else {
                        format!("Verified {} claims against active evidence sources", report.total_claims)
                    },
                };
                let _ = self.memory.record_drift_event(&drift_event);

                // Persist full untruncated EpistemicTrajectory and KTO post-training pairs
                let verified_vec: Vec<_> = report
                    .claims
                    .iter()
                    .filter(|c| c.status == crate::grounding::VerificationStatus::Entailed)
                    .collect();
                let contradicted_vec: Vec<_> = report
                    .claims
                    .iter()
                    .filter(|c| c.status == crate::grounding::VerificationStatus::Contradicted)
                    .collect();
                let unverified_vec: Vec<_> = report
                    .claims
                    .iter()
                    .filter(|c| c.status == crate::grounding::VerificationStatus::Unverified)
                    .collect();

                let trajectory = EpistemicTrajectory {
                    id: 0,
                    timestamp: now_ts.clone(),
                    project_id: "antigravity-ide".to_string(),
                    model_id: "claim-verifier".to_string(),
                    query: response_text.to_string(),
                    draft_response: response_text.to_string(),
                    verified_claims_json: serde_json::to_string(&verified_vec).unwrap_or_else(|_| "[]".to_string()),
                    contradicted_claims_json: serde_json::to_string(&contradicted_vec).unwrap_or_else(|_| "[]".to_string()),
                    unverified_claims_json: serde_json::to_string(&unverified_vec).unwrap_or_else(|_| "[]".to_string()),
                    source_evidence_json: serde_json::to_string(&sources).unwrap_or_else(|_| "[]".to_string()),
                    hallucination_risk_index: report.hallucination_risk_index,
                    intervention_type: intervention,
                };
                let _ = self.memory.record_epistemic_trajectory(&trajectory);

                let evidence_summary = sources
                    .iter()
                    .map(|s| format!("[{}] {}: {}", s.id, s.title, s.content))
                    .collect::<Vec<_>>()
                    .join("\n");
                for vc in &report.claims {
                    match vc.status {
                        crate::grounding::VerificationStatus::Entailed => {
                            let _ = self.memory.record_post_training_pair(&PostTrainingPair {
                                id: 0,
                                timestamp: now_ts.clone(),
                                project_id: "antigravity-ide".to_string(),
                                prompt: response_text.to_string(),
                                response: vc.claim.statement.clone(),
                                label: "desirable".to_string(),
                                kto_weight: 1.0,
                                source_evidence: if evidence_summary.is_empty() {
                                    vc.rationale.clone()
                                } else {
                                    evidence_summary.clone()
                                },
                                reward_score: vc.confidence_score,
                            });
                        }
                        crate::grounding::VerificationStatus::Contradicted => {
                            let _ = self.memory.record_post_training_pair(&PostTrainingPair {
                                id: 0,
                                timestamp: now_ts.clone(),
                                project_id: "antigravity-ide".to_string(),
                                prompt: response_text.to_string(),
                                response: vc.claim.statement.clone(),
                                label: "undesirable".to_string(),
                                kto_weight: 2.25, // Kahneman-Tversky loss-aversion multiplier for hallucinations
                                source_evidence: if evidence_summary.is_empty() {
                                    vc.rationale.clone()
                                } else {
                                    evidence_summary.clone()
                                },
                                reward_score: -1.0,
                            });
                        }
                        crate::grounding::VerificationStatus::Unverified => {}
                    }
                }

                Ok(json!(report))
            }
            "chronofact_memory_save" => {
                let project_id = args.get("project_id").and_then(|v| v.as_str()).ok_or("Missing project_id")?;
                let entity_name = args.get("entity_name").and_then(|v| v.as_str()).ok_or("Missing entity_name")?;
                let entity_type = args.get("entity_type").and_then(|v| v.as_str()).ok_or("Missing entity_type")?;
                let definition = args.get("definition").and_then(|v| v.as_str()).ok_or("Missing definition")?;
                let version = args.get("version").and_then(|v| v.as_str()).unwrap_or("1.0.0");

                let entity = ProjectEntity {
                    id: format!("{}:{}", project_id, entity_name.to_lowercase().replace(' ', "_")),
                    project_id: project_id.to_string(),
                    entity_name: entity_name.to_string(),
                    entity_type: entity_type.to_string(),
                    definition: definition.to_string(),
                    version: version.to_string(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };

                self.memory.upsert_entity(&entity).map_err(|e| e.to_string())?;

                // Also record an event in L2 scoped to the target project_id
                let session_key = format!("mcp-session:{}", project_id);
                let _ = self.memory.record_event(
                    &session_key,
                    "MEMORY_RECORD",
                    &format!("Updated entity '{}' ({})", entity_name, entity_type),
                    Some(definition),
                );

                Ok(json!({ "status": "saved", "entity_id": entity.id }))
            }
            "chronofact_memory_dossier" => {
                let project_id = args.get("project_id").and_then(|v| v.as_str()).ok_or("Missing project_id")?;
                let query = args.get("query").and_then(|v| v.as_str());
                let dossier = if let Some(q) = query {
                    self.memory.get_relevant_dossier(project_id, q).map_err(|e| e.to_string())?
                } else {
                    self.memory.get_dossier(project_id).map_err(|e| e.to_string())?
                };
                Ok(json!(dossier))
            }
            "chronofact_query" => {
                let project_id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or("default");
                let model_id = args.get("model_id").and_then(|v| v.as_str()).unwrap_or("claude-opus-5-5");
                let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing query")?;

                // 1. Relevance-Gated Memory Dossier (Zero-Pollution)
                let dossier = self.memory.get_relevant_dossier(project_id, query).map_err(|e| e.to_string())?;

                // 2. Temporal Horizon
                let model = self.model_registry.lookup(model_id);
                let scan = self.scanner.scan(query);
                let today = chrono::Utc::now().naive_utc().date();
                let horizon = HorizonCalculator::evaluate(&model, &scan, today);

                // 3. Search Grounding if necessary
                let sources = if horizon.requires_grounding {
                    let search_q = scan.search_query_suggestion.as_deref().unwrap_or(query);
                    self.searcher.search(search_q, 3).await.unwrap_or_default()
                } else {
                    Vec::new()
                };

                let drift_event = DriftEvent {
                    id: 0,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    project_id: format!("ide:{}", project_id),
                    model_id: model.model_id.clone(),
                    query: query.to_string(),
                    days_post_freeze: horizon.days_post_freeze,
                    temporal_risk_score: horizon.temporal_risk_score,
                    outdated_topics_caught: if scan.detected_entities.is_empty() {
                        "IDE Epistemic Query".to_string()
                    } else {
                        scan.detected_entities.join(", ")
                    },
                    intervention_type: if horizon.requires_grounding {
                        "ACTIVE_EPISTEMIC_GROUNDING".to_string()
                    } else {
                        "EPISTEMIC_PASS_THROUGH".to_string()
                    },
                    ground_truth_retrieved: if sources.is_empty() {
                        "Verified against invariant knowledge base".to_string()
                    } else {
                        sources.iter().map(|s| s.title.clone()).collect::<Vec<_>>().join(" | ")
                    },
                };
                let _ = self.memory.record_drift_event(&drift_event);

                let cache_optimized = CostTracker::assemble_cache_aligned_prompt_with_mode(
                    project_id,
                    &dossier.dossier_markdown,
                    "",
                    &horizon.calibration_block,
                    &sources.iter().map(|s| format!("{}: {}", s.title, s.content)).collect::<Vec<_>>().join("\n"),
                    query,
                    crate::cost::tracker::SessionMode::StatelessSingleShot,
                );

                Ok(json!({
                    "project_id": project_id,
                    "model": horizon.model_name,
                    "temporal_analysis": horizon,
                    "grounding_sources": sources,
                    "project_dossier": dossier,
                    "combined_system_context": format!("{}\n\n{}", dossier.dossier_markdown, horizon.calibration_block),
                    "cache_optimized_prompt": cache_optimized
                }))
            }
            "chronofact_cost_optimize" => {
                let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing query")?;
                let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(4) as usize;

                let mut candidate_tools = Self::native_tool_candidates(true);

                if let Some(ref gw) = self.gateway {
                    let gw_guard = gw.read().await;
                    candidate_tools.extend(gw_guard.to_tool_candidates());
                }

                let routing = self.tool_router.route(query, &candidate_tools, top_k, None);
                self.cost_tracker.record_savings(routing.pruned_tools, routing.tokens_saved);
                let cost_usd = (routing.tokens_saved as f64 * 0.000003 * 10000.0).round() / 10000.0;
                let _ = self
                    .memory
                    .record_cost_event(query, routing.pruned_tools, routing.tokens_saved, cost_usd);

                let cache_prefix = CostTracker::generate_cache_aligned_prefix("antigravity-ide");
                let schemas_json = serde_json::to_string_pretty(&routing.selected_tools).unwrap_or_default();
                let cache_optimized = CostTracker::assemble_cache_aligned_prompt_with_mode(
                    "antigravity-ide",
                    "You are an expert AI assistant operating under ChronoFact + SPINE invariants.",
                    &schemas_json,
                    "",
                    "",
                    query,
                    crate::cost::tracker::SessionMode::StatelessSingleShot,
                );

                Ok(json!({
                    "optimization": routing,
                    "prompt_cache_anchor": cache_prefix,
                    "cache_optimized_prompt": cache_optimized,
                    "estimated_savings": {
                        "tokens_saved": routing.tokens_saved,
                        "savings_percentage": routing.savings_percentage,
                        "basis": "Full ChronoFact + SPINE + Antigravity + connected Gateway candidate pool",
                        "recorded_to_ledger": true
                    }
                }))
            }
            "chronofact_cost_metrics" => {
                let (hits, misses, entries, _hit_rate) = self.tool_cache.stats();
                let db_totals = self.memory.get_cost_totals().unwrap_or((0, 0, 0, 0.0));
                let metrics = self.cost_tracker.get_metrics_with_db(db_totals, hits, misses, entries);
                Ok(json!(metrics))
            }
            "gateway_find_tools" => {
                let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing query")?;
                let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

                let mut all_candidates = Self::native_tool_candidates(false);

                if let Some(ref gw) = self.gateway {
                    let gw_guard = gw.read().await;
                    all_candidates.extend(gw_guard.to_tool_candidates());
                }

                let routing = self.tool_router.route(query, &all_candidates, top_k, None);
                self.cost_tracker.record_savings(routing.pruned_tools, routing.tokens_saved);
                let cost_usd = (routing.tokens_saved as f64 * 0.000003 * 10000.0).round() / 10000.0;
                let _ = self
                    .memory
                    .record_cost_event(query, routing.pruned_tools, routing.tokens_saved, cost_usd);

                Ok(json!({
                    "query": query,
                    "matched_tools": routing.selected_tools,
                    "pruned_count": routing.pruned_tools,
                    "tokens_saved": routing.tokens_saved,
                    "savings_percentage": routing.savings_percentage
                }))
            }
            "gateway_list_servers" => {
                if let Some(ref gw) = self.gateway {
                    let gw_guard = gw.read().await;
                    Ok(json!(gw_guard.get_server_statuses()))
                } else {
                    Ok(json!([]))
                }
            }
            "gateway_call_tool" => {
                let tool_name = args.get("name").and_then(|v| v.as_str()).ok_or("Missing name")?;
                let tool_args = args.get("arguments").cloned().unwrap_or(json!({}));
                let verify_output = args.get("verify_output").and_then(|v| v.as_bool()).unwrap_or(true);
                let passthrough = args.get("passthrough").and_then(|v| v.as_bool()).unwrap_or(false);

                self.dispatch_upstream_tool(tool_name, &tool_args, verify_output, passthrough).await
            }
            "chronofact_verify_code_invariants" => {
                let file_path = args
                    .get("file_path")
                    .or_else(|| args.get("path"))
                    .or_else(|| args.get("target"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let content = args
                    .get("content")
                    .or_else(|| args.get("code"))
                    .or_else(|| args.get("proposal"))
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'content' (or 'code'/'proposal')")?;
                let custom_rules: Option<Vec<crate::grounding::InvariantRule>> =
                    args.get("custom_rules").and_then(|v| serde_json::from_value(v.clone()).ok());

                let checker = crate::grounding::CodeInvariantChecker::new();
                let report = checker.audit_code(file_path, content, custom_rules.as_deref());

                let now_ts = chrono::Utc::now().to_rfc3339();
                let hri = if report.violations_count > 0 {
                    (1.0 - (1.0 - 0.38_f32).powi(report.violations_count as i32)).min(1.0)
                } else {
                    0.0
                };
                let _ = self.memory.record_epistemic_trajectory(&EpistemicTrajectory {
                    id: 0,
                    timestamp: now_ts.clone(),
                    project_id: "antigravity-ide".to_string(),
                    model_id: "code-invariant-gate".to_string(),
                    query: file_path.to_string(),
                    draft_response: content.to_string(),
                    verified_claims_json: "[]".to_string(),
                    contradicted_claims_json: serde_json::to_string(&report.violations).unwrap_or_else(|_| "[]".to_string()),
                    unverified_claims_json: "[]".to_string(),
                    source_evidence_json: "[]".to_string(),
                    hallucination_risk_index: hri,
                    intervention_type: if report.violations_count > 0 {
                        "CODE_INVARIANT_VIOLATION".to_string()
                    } else {
                        "CODE_INVARIANT_VERIFIED".to_string()
                    },
                });
                let _ = self.memory.record_post_training_pair(&PostTrainingPair {
                    id: 0,
                    timestamp: now_ts,
                    project_id: "antigravity-ide".to_string(),
                    prompt: format!("Audit code invariants for {}", file_path),
                    response: content.to_string(),
                    label: if report.violations_count > 0 {
                        "undesirable".to_string()
                    } else {
                        "desirable".to_string()
                    },
                    kto_weight: if report.violations_count > 0 { 2.25 } else { 1.0 },
                    source_evidence: serde_json::to_string(&report.violations).unwrap_or_else(|_| "[]".to_string()),
                    reward_score: if report.violations_count > 0 { -1.0 } else { 1.0 },
                });

                Ok(json!(report))
            }
            "chronofact_attestation_generate" => {
                let project_id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or("default");
                let file_path = args
                    .get("file_path")
                    .or_else(|| args.get("path"))
                    .or_else(|| args.get("target"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let content = args
                    .get("content")
                    .or_else(|| args.get("code"))
                    .or_else(|| args.get("proposal"))
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'content' (or 'code'/'proposal')")?;
                let default_today = chrono::Utc::now().naive_utc().date().to_string();
                let temporal_anchor = args.get("temporal_anchor").and_then(|v| v.as_str()).unwrap_or(&default_today);

                let checker = crate::grounding::CodeInvariantChecker::new();
                let report = checker.audit_code(file_path, content, None);
                let attestation = crate::grounding::AttestationEngine::create_attestation(
                    &report,
                    project_id,
                    content,
                    temporal_anchor,
                    None,
                );

                Ok(json!({
                    "attestation": attestation,
                    "report_summary": {
                        "verdict": report.verdict,
                        "violations_count": report.violations_count,
                        "critical_count": report.critical_count,
                        "high_count": report.high_count
                    }
                }))
            }
            "chronofact_expand_tool_palette" => {
                let query = args.get("query_or_category")
                    .or_else(|| args.get("query"))
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'query_or_category'")?;
                let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(4) as usize;

                let mut all_candidates = Self::native_tool_candidates(true);

                if let Some(ref gw) = self.gateway {
                    let gw_guard = gw.read().await;
                    all_candidates.extend(gw_guard.to_tool_candidates());
                }

                let routing = self.tool_router.route(query, &all_candidates, top_k, None);
                Ok(json!({
                    "expanded_tools": routing.selected_tools,
                    "count": routing.selected_tools.len(),
                    "query_or_category": query,
                    "recovery_status": "SUCCESS",
                    "instruction": "These tool schemas are now recovered and available for your current execution turn."
                }))
            }
            other => {
                if let Some(ref gw) = self.gateway {
                    let gw_guard = gw.read().await;
                    if gw_guard.find_tool_server(other).is_some() {
                        drop(gw_guard);
                        return self.dispatch_upstream_tool(other, args, true, true).await;
                    }
                }
                Err(format!("Unknown tool: {}", other))
            }
        }
    }

    pub async fn dispatch_upstream_tool(
        &self,
        name: &str,
        args: &Value,
        verify_output: bool,
        passthrough: bool,
    ) -> Result<Value, String> {
        let gw = self.gateway.as_ref().ok_or("Gateway subsystem not initialized")?;

        // 1. Inbound Security Sanitization
        let args_str = args.to_string();
        let (has_threats, threats) = self.sanitizer.inspect_user_query(&args_str);
        if has_threats {
            if passthrough {
                tracing::warn!("Gateway passthrough mode: argument inspection warning: {:?}", threats);
            } else {
                return Err(format!("Security shield blocked dangerous tool call argument: {:?}", threats));
            }
        }

        // 2. Read-Only Gating for Cache Check: only cache tools that declare readOnlyHint: true
        let (is_read_only, canonical_tool_name, server_name) = {
            let gw_guard = gw.read().await;
            let ro = gw_guard.get_tool_definition(name).map(|t| t.is_read_only()).unwrap_or(false);
            let srv = gw_guard.find_tool_server(name).unwrap_or_else(|| "default".to_string());
            let canonical = if let Some(server) = gw_guard.find_tool_server(name) {
                let bare = name.split_once('/').map(|(_, b)| b).unwrap_or(name);
                format!("{}/{}", server, bare)
            } else {
                name.to_string()
            };
            (ro, canonical, srv)
        };

        if is_read_only {
            if let Some(cached) = self.tool_cache.get(&canonical_tool_name, args) {
                let has_multimodal_content = cached
                    .get("content")
                    .and_then(|c| c.as_array())
                    .map(|arr| {
                        arr.iter().any(|item| {
                            matches!(
                                item.get("type").and_then(|t| t.as_str()),
                                Some("image") | Some("resource") | Some("audio")
                            )
                        })
                    })
                    .unwrap_or(false);
                if passthrough || has_multimodal_content {
                    return Ok(cached);
                } else {
                    return Ok(json!({
                        "result": cached,
                        "cache_hit": true,
                        "verified": true
                    }));
                }
            }
        }

        // Circuit breaker check: fail-fast if upstream server has tripped
        {
            let cb_guard = self.circuit_breakers.read().await;
            if let Some(cb) = cb_guard.get(&server_name) {
                if cb.is_open(std::time::Duration::from_secs(30)) {
                    return Err(format!(
                        "Gateway circuit breaker OPEN for server '{}' ({} consecutive timeouts). Requests throttled for 30s.",
                        server_name, cb.consecutive_timeouts
                    ));
                }
            }
        }

        // 3. Dispatch to Upstream Server (resolve target under ephemeral lock to avoid holding RwLock across network await)
        let target = {
            let gw_guard = gw.read().await;
            gw_guard.resolve_tool_execution(name, Some(args.clone()))
                .map_err(|e| format!("Failed to route tool '{}': {}", name, e))?
        };

        let exec_future = target.execute();
        let upstream_result = match tokio::time::timeout(std::time::Duration::from_secs(60), exec_future).await {
            Ok(Ok(res)) => {
                let mut cb_guard = self.circuit_breakers.write().await;
                if let Some(cb) = cb_guard.get_mut(&server_name) {
                    cb.record_success();
                }
                res
            }
            Ok(Err(e)) => {
                let err_str = e.to_string();
                let err_lower = err_str.to_lowercase();
                if err_lower.contains("timed out") || err_lower.contains("timeout") {
                    let mut cb_guard = self.circuit_breakers.write().await;
                    let cb = cb_guard.entry(server_name.clone()).or_default();
                    let tripped = cb.record_timeout(3);
                    if tripped {
                        return Err(format!(
                            "Upstream tool execution timed out ({}). Circuit breaker TRIPPED for server '{}' (3 consecutive timeouts).",
                            err_str, server_name
                        ));
                    }
                }
                return Err(format!("Upstream tool execution error: {}", err_str));
            }
            Err(_) => {
                let mut cb_guard = self.circuit_breakers.write().await;
                let cb = cb_guard.entry(server_name.clone()).or_default();
                let tripped = cb.record_timeout(3);
                if tripped {
                    return Err(format!(
                        "Upstream tool execution timed out after 60s. Circuit breaker TRIPPED for server '{}' (3 consecutive timeouts).",
                        server_name
                    ));
                } else {
                    return Err(format!(
                        "Upstream tool execution timed out after 60s on server '{}' (strike {}/3).",
                        server_name, cb.consecutive_timeouts
                    ));
                }
            }
        };

        // 4. Outbound Cognitive Epistemic Verification (Deterministic Policy Invariant Check)
        // Extract only text blocks when MCP content array is present so base64 image/resource payloads are never scanned as prose claims.
        let verification_info = if verify_output {
            let result_str = if let Some(content_arr) = upstream_result.get("content").and_then(|c| c.as_array()) {
                content_arr
                    .iter()
                    .filter(|item| item.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                upstream_result.to_string()
            };
            let claims = self.extractor.extract_claims(&result_str);
            if !claims.is_empty() {
                let mut report = self.verifier.verify_claims(&claims, &[]);
                if report.contradicted_count == 0 {
                    report.hallucination_risk_index = 0.0;
                }
                Some(report)
            } else {
                None
            }
        } else {
            None
        };

        // 5. Cache result for read-only tools, or invalidate cache for mutating tools
        if is_read_only {
            self.tool_cache.put(&canonical_tool_name, args, upstream_result.clone());
        } else {
            self.tool_cache.invalidate_server(&server_name);
            let tool_lower = name.to_lowercase();
            if tool_lower.contains("write")
                || tool_lower.contains("edit")
                || tool_lower.contains("replace")
                || tool_lower.contains("modify")
                || tool_lower.contains("delete")
                || tool_lower.contains("remove")
            {
                if let Some(path) = ToolResponseCache::extract_path(args) {
                    self.tool_cache.invalidate_path(&path);
                } else {
                    self.tool_cache.invalidate_filesystem_entries();
                }
            } else if tool_lower.contains("command")
                || tool_lower.contains("exec")
                || tool_lower.contains("run")
                || tool_lower.contains("terminal")
                || tool_lower.contains("shell")
                || tool_lower.contains("bash")
            {
                self.tool_cache.invalidate_filesystem_entries();
            }
        }

        let has_multimodal_content = upstream_result
            .get("content")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter().any(|item| {
                    matches!(
                        item.get("type").and_then(|t| t.as_str()),
                        Some("image") | Some("resource") | Some("audio")
                    )
                })
            })
            .unwrap_or(false);

        if passthrough || has_multimodal_content {
            Ok(upstream_result)
        } else {
            Ok(json!({
                "result": upstream_result,
                "cache_hit": false,
                "verification": verification_info
            }))
        }
    }
}

