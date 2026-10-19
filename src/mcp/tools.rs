use serde_json::{json, Value};

pub fn list_tools() -> Value {
    json!({
        "tools": [
            {
                "name": "chronofact_temporal_check",
                "description": "Checks query against model knowledge cutoff/training freeze. Detects temporal drift, retired models, and emits a calibration anchor.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_id": {
                            "type": "string",
                            "description": "Model ID (e.g. 'gemini-4-argon', 'claude-opus-5-5', 'claude-sonnet-5-5', 'gpt-6-astra', 'gpt-6-1-sol', 'grok-4-7')"
                        },
                        "query": {
                            "type": "string",
                            "description": "User query or prompt"
                        },
                        "compact": {
                            "type": "boolean",
                            "description": "Return compact token-saving anchor payload"
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "chronofact_ground_query",
                "description": "Real-time web grounding with SSRF firewall and indirect prompt-injection sanitization.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query to ground"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Max evidence chunks (default: 4)"
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "chronofact_verify_claims",
                "description": "Extracts atomic propositions and fenced code blocks, verifies against 2026 ground truth and code invariants, and scores hallucination risk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "response_text": {
                            "type": "string",
                            "description": "Response text or code to verify"
                        },
                        "sources": {
                            "type": "array",
                            "description": "Optional evidence chunks",
                            "items": { "type": "object" }
                        }
                    },
                    "required": ["response_text"]
                }
            },
            {
                "name": "chronofact_memory_save",
                "description": "Persists an architectural invariant, tech stack version, or rule into L3 semantic SQLite memory.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Project ID" },
                        "entity_name": { "type": "string", "description": "Entity or rule name" },
                        "entity_type": { "type": "string", "description": "TECH_STACK, API_CONTRACT, ARCHITECTURE_RULE, DATABASE_SCHEMA" },
                        "definition": { "type": "string", "description": "Invariant specification" },
                        "version": { "type": "string", "description": "Version string" }
                    },
                    "required": ["project_id", "entity_name", "entity_type", "definition"]
                }
            },
            {
                "name": "chronofact_memory_dossier",
                "description": "Retrieves Project Truth Dossier with relevance-gated Zero-Pollution Guard.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Project ID" },
                        "query": { "type": "string", "description": "Optional query for relevance gating" }
                    },
                    "required": ["project_id"]
                }
            },
            {
                "name": "chronofact_query",
                "description": "Full-cycle epistemic pipeline: temporal horizon + search grounding + L3 memory dossier + KV cache isolation.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Project ID" },
                        "model_id": { "type": "string", "description": "Model ID" },
                        "query": { "type": "string", "description": "User prompt" }
                    },
                    "required": ["project_id", "model_id", "query"]
                }
            },
            {
                "name": "chronofact_cost_optimize",
                "description": "Routes tools via BM25/TF-IDF + subword expansion, pruning irrelevant schemas by 70-90% with KV cache prefix isolation.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Task query for tool routing" },
                        "top_k": { "type": "integer", "description": "Max tools to retain (default: 4)" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "chronofact_cost_metrics",
                "description": "Returns session token savings, tool cache hit rate, and estimated USD saved.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "gateway_find_tools",
                "description": "Searches native and upstream MCP tools via BM25/TF-IDF semantic routing.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Capability query" },
                        "top_k": { "type": "integer", "description": "Tools to return (default: 5)" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "gateway_call_tool",
                "description": "Executes an upstream MCP tool with security sanitization, FNV-1a read-only caching, and claim verification.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Tool name or server/tool FQN" },
                        "arguments": { "type": "object", "description": "Tool arguments" },
                        "verify_output": { "type": "boolean", "description": "Audit output claims (default: true)" },
                        "passthrough": { "type": "boolean", "description": "Return raw content (default: false)" }
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "gateway_list_servers",
                "description": "Lists connected upstream MCP servers, tool counts, and health.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "chronofact_verify_code_invariants",
                "description": "Audits source code/diffs across Rust, TS/JS/React, Python, C/C++/CUDA, Go, SQL, and PowerShell against 12 security, memory-safety, and anti-hallucination invariants.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_path": { "type": "string", "description": "Target file path" },
                        "content": { "type": "string", "description": "Source code or diff to audit" },
                        "custom_rules": { "type": "array", "description": "Optional custom rules", "items": { "type": "object" } }
                    },
                    "required": ["file_path", "content"]
                }
            },
            {
                "name": "chronofact_attestation_generate",
                "description": "Generates an HMAC-SHA256 Invariant Attestation certificate (CF-2026-v1) binding code SHA-256, temporal anchor, and audit verdict.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Project ID" },
                        "file_path": { "type": "string", "description": "Verified file path" },
                        "content": { "type": "string", "description": "Audited content" },
                        "temporal_anchor": { "type": "string", "description": "Evaluation date" }
                    },
                    "required": ["project_id", "file_path", "content"]
                }
            },
            {
                "name": "chronofact_expand_tool_palette",
                "description": "Recovers additional tool schemas on demand via BM25/TF-IDF when operating with a pruned tool palette.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query_or_category": { "type": "string", "description": "Operation or category needed" },
                        "top_k": { "type": "integer", "description": "Schemas to recover (default: 4)" }
                    },
                    "required": ["query_or_category"]
                }
            }
        ]
    })
}

/// Returns the `required` field names declared in `list_tools()` for `tool_name`.
pub fn required_fields(tool_name: &str) -> Vec<String> {
    list_tools()
        .get("tools")
        .and_then(|t| t.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(tool_name))
        })
        .and_then(|t| t.get("inputSchema"))
        .and_then(|s| s.get("required"))
        .and_then(|r| r.as_array())
        .map(|r| r.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Validates every schema-declared required field of `tool_name` up front and returns ONE error
/// listing all fields that are absent, null, empty/whitespace-only, or not a string.
pub fn validate_required_string_args(tool_name: &str, args: &Value) -> Result<(), String> {
    let required = required_fields(tool_name);
    let mut missing: Vec<String> = Vec::new();
    let mut not_string: Vec<String> = Vec::new();
    for field in &required {
        match args.get(field) {
            None | Some(Value::Null) => missing.push(field.clone()),
            Some(Value::String(s)) if s.trim().is_empty() => missing.push(field.clone()),
            Some(Value::String(_)) => {}
            Some(_) => not_string.push(field.clone()),
        }
    }
    if missing.is_empty() && not_string.is_empty() {
        return Ok(());
    }
    let mut parts = Vec::new();
    if !missing.is_empty() {
        parts.push(format!("missing required field(s): {}", missing.join(", ")));
    }
    if !not_string.is_empty() {
        parts.push(format!("field(s) must be non-empty strings: {}", not_string.join(", ")));
    }
    Err(format!(
        "Invalid arguments for {}: {}. Required fields: {}.",
        tool_name,
        parts.join("; "),
        required.join(", ")
    ))
}
