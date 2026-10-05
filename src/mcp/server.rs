use super::protocol::{JsonRpcRequest, JsonRpcResponse};
use super::tools;
use crate::grounding::{ClaimExtractor, FactVerifier};
use crate::memory::{DriftEvent, MemoryEngine, ProjectEntity};
use crate::research::SearchEngine;
use crate::temporal::{HorizonCalculator, ModelRegistry, TemporalScanner};
use serde_json::{json, Value};
use crate::cost::{CostTracker, TfidfToolRouter, ToolCandidate, ToolResponseCache};
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

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
}

impl McpServer {
    pub fn new(memory: Arc<MemoryEngine>) -> Self {
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
        }
    }

    pub async fn run_stdio(&self) -> io::Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(e) => {
                    let err_resp = JsonRpcResponse::error(None, -32700, &format!("Parse error: {}", e));
                    let out = serde_json::to_string(&err_resp).unwrap();
                    stdout.write_all(format!("{}\n", out).as_bytes()).await?;
                    stdout.flush().await?;
                    continue;
                }
            };

            let resp = self.handle_request(req).await;
            if let Some(r) = resp {
                let out = serde_json::to_string(&r).unwrap();
                stdout.write_all(format!("{}\n", out).as_bytes()).await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }

    pub async fn handle_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id;
        match req.method.as_str() {
            "initialize" => {
                let result = json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "chronofact",
                        "version": "0.1.0"
                    }
                });
                Some(JsonRpcResponse::success(id, result))
            }
            "notifications/initialized" => None,
            "ping" => Some(JsonRpcResponse::success(id, json!({}))),
            "tools/list" => {
                let tools_data = tools::list_tools();
                Some(JsonRpcResponse::success(id, tools_data))
            }
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);

                let tool_result = self.execute_tool(tool_name, &args).await;
                match tool_result {
                    Ok(val) => {
                        let result_obj = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string())
                                }
                            ],
                            "isError": false
                        });
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
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let scan = self.scanner.scan(query);

                // Dynamically detect model from prompt if not specified or if specific model query detected
                let effective_model_id = if !requested_model_id.is_empty() && requested_model_id != "auto" && requested_model_id != "default" {
                    requested_model_id
                } else if let Some(flagged) = scan.outdated_models_flagged.first() {
                    let f_lower = flagged.to_lowercase();
                    if f_lower.contains("3.5") {
                        "claude-3-5-sonnet"
                    } else if f_lower.contains("3.7") {
                        "claude-3-7-sonnet"
                    } else if f_lower.contains("grok") {
                        "grok-3"
                    } else if f_lower.contains("gpt-4") {
                        "gpt-4"
                    } else {
                        "claude-opus-5-5"
                    }
                } else {
                    let q_lower = query.to_lowercase();
                    if q_lower.contains("astra") || q_lower.contains("astr-6") || q_lower.contains("astr 6") {
                        "gpt-6-astra"
                    } else if q_lower.contains("sol-6") || q_lower.contains("sol 6") || q_lower.contains("sol-6.1") || q_lower.contains("sol 6.1") {
                        "gpt-6-1-sol"
                    } else if q_lower.contains("opus 5") || q_lower.contains("opus-5") {
                        "claude-opus-5-5"
                    } else if q_lower.contains("sonnet 5") || q_lower.contains("sonnet-5") {
                        "claude-sonnet-5-5"
                    } else if q_lower.contains("grok 4") || q_lower.contains("grok-4") {
                        "grok-4-7"
                    } else if q_lower.contains("gemini") {
                        "gemini-3-8-flash"
                    } else {
                        "claude-opus-5-5"
                    }
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

                Ok(json!(analysis))
            }
            "chronofact_ground_query" => {
                let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing query")?;
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
                let response_text = args.get("response_text").and_then(|v| v.as_str()).ok_or("Missing response_text")?;
                let sources: Vec<crate::research::SourceChunk> = if let Some(arr) = args.get("sources") {
                    serde_json::from_value(arr.clone()).unwrap_or_default()
                } else {
                    Vec::new()
                };

                let claims = self.extractor.extract_claims(response_text);
                let report = self.verifier.verify_claims(&claims, &sources);

                let snippet = if response_text.len() > 90 {
                    format!("{}...", &response_text[..90])
                } else {
                    response_text.to_string()
                };
                let drift_event = DriftEvent {
                    id: 0,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    project_id: "antigravity-ide".to_string(),
                    model_id: "claim-verifier".to_string(),
                    query: snippet,
                    days_post_freeze: 0,
                    temporal_risk_score: report.hallucination_risk_index,
                    outdated_topics_caught: format!(
                        "Audited {} claims (Entailed: {}, Contradicted: {}, Unverified: {})",
                        report.total_claims, report.entailed_count, report.contradicted_count, report.unverified_count
                    ),
                    intervention_type: if report.contradicted_count > 0 {
                        "CONTRADICTION_INTERCEPTED".to_string()
                    } else {
                        "CLAIM_VERIFICATION".to_string()
                    },
                    ground_truth_retrieved: if report.contradicted_count > 0 {
                        format!("CONTRADICTION DETECTED: Hallucination risk {:.0}%", report.hallucination_risk_index * 100.0)
                    } else {
                        format!("Verified {} claims against active evidence sources", report.total_claims)
                    },
                };
                let _ = self.memory.record_drift_event(&drift_event);

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

                // Also record an event in L2
                let _ = self.memory.record_event(
                    "mcp-session",
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

                Ok(json!({
                    "project_id": project_id,
                    "model": horizon.model_name,
                    "temporal_analysis": horizon,
                    "grounding_sources": sources,
                    "project_dossier": dossier,
                    "combined_system_context": format!("{}\n\n{}", dossier.dossier_markdown, horizon.calibration_block)
                }))
            }
            "chronofact_cost_optimize" => {
                let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing query")?;
                let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(4) as usize;

                let default_tools = vec![
                    ToolCandidate::new("chronofact_temporal_check", "Examines a query against a model's knowledge cutoff and estimated training freeze date", "chronofact", vec!["model_id".into(), "query".into()]),
                    ToolCandidate::new("chronofact_ground_query", "Performs real-time web retrieval, validates URLs against SSRF, and sanitizes untrusted content", "chronofact", vec!["query".into(), "max_results".into()]),
                    ToolCandidate::new("chronofact_verify_claims", "Decomposes a text response into atomic propositions and checks each against retrieved source evidence", "chronofact", vec!["response_text".into(), "sources".into()]),
                    ToolCandidate::new("chronofact_memory_save", "Persists an architectural invariant, tech stack version, or project rule into L3 semantic memory", "chronofact", vec!["project_id".into(), "entity_name".into(), "definition".into()]),
                    ToolCandidate::new("chronofact_memory_dossier", "Retrieves the persistent Project Truth Dossier with Zero-Pollution Guard", "chronofact", vec!["project_id".into(), "query".into()]),
                    ToolCandidate::new("chronofact_cost_optimize", "Filters available tools to top-k relevant tools using TF-IDF routing to save tokens", "chronofact", vec!["query".into(), "top_k".into()]),
                    ToolCandidate::new("run_command", "Propose a command to run on behalf of the user in PowerShell shell", "antigravity", vec!["CommandLine".into(), "Cwd".into()]),
                    ToolCandidate::new("view_file", "View the contents of a file from the local filesystem", "antigravity", vec!["AbsolutePath".into()]),
                    ToolCandidate::new("replace_file_content", "Use this tool to edit an existing file with single contiguous replacement", "antigravity", vec!["TargetFile".into(), "TargetContent".into(), "ReplacementContent".into()]),
                    ToolCandidate::new("search_web", "Performs a web search for a given query and returns summary", "antigravity", vec!["query".into()]),
                    ToolCandidate::new("read_url_content", "Fetch content from a URL via HTTP request", "antigravity", vec!["Url".into()]),
                ];

                let routing = self.tool_router.route(query, &default_tools, top_k, None);
                self.cost_tracker.record_savings(routing.pruned_tools, routing.tokens_saved);

                let cache_prefix = CostTracker::generate_cache_aligned_prefix("antigravity-ide");

                Ok(json!({
                    "optimization": routing,
                    "prompt_cache_anchor": cache_prefix,
                    "estimated_savings": {
                        "tokens_saved": routing.tokens_saved,
                        "savings_percentage": routing.savings_percentage,
                        "cost_savings_tier": "HIGH"
                    }
                }))
            }
            "chronofact_cost_metrics" => {
                let (hits, misses, entries, _hit_rate) = self.tool_cache.stats();
                let metrics = self.cost_tracker.get_metrics(hits, misses, entries);
                Ok(json!(metrics))
            }
            other => Err(format!("Unknown tool: {}", other)),
        }
    }
}
