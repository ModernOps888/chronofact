use serde_json::{json, Value};

pub fn list_tools() -> Value {
    json!({
        "tools": [
            {
                "name": "chronofact_temporal_check",
                "description": "Examines a query against a model's knowledge cutoff and estimated training freeze date. Detects temporal risk, newly released tools, and generates an un-jammable calibration prompt.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_id": {
                            "type": "string",
                            "description": "Target model identifier (e.g. 'claude-opus-5-5', 'claude-3-7-sonnet', 'gemini-3-8-flash', 'gpt-4o')"
                        },
                        "query": {
                            "type": "string",
                            "description": "The user query or prompt to check for temporal drift"
                        }
                    },
                    "required": ["model_id", "query"]
                }
            },
            {
                "name": "chronofact_ground_query",
                "description": "Performs real-time web retrieval, validates URLs against SSRF, and sanitizes untrusted content against indirect prompt injections.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query to ground with real-time web documentation"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum number of search evidence chunks (default: 4)"
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "chronofact_verify_claims",
                "description": "Decomposes a text response into atomic propositions, checks each against retrieved source evidence using deterministic lexical and invariant rules, and scores hallucination risk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "response_text": {
                            "type": "string",
                            "description": "The LLM response text or code commentary to audit for factual accuracy"
                        },
                        "sources": {
                            "type": "array",
                            "description": "List of source chunks to verify against",
                            "items": { "type": "object" }
                        }
                    },
                    "required": ["response_text"]
                }
            },
            {
                "name": "chronofact_memory_save",
                "description": "Persists an architectural invariant, tech stack version, or project rule into L3 semantic memory so it is remembered across all chats.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": {
                            "type": "string",
                            "description": "Identifier for the project"
                        },
                        "entity_name": {
                            "type": "string",
                            "description": "Name of the entity, package, schema, or rule"
                        },
                        "entity_type": {
                            "type": "string",
                            "description": "Type: TECH_STACK, API_CONTRACT, ARCHITECTURE_RULE, DATABASE_SCHEMA"
                        },
                        "definition": {
                            "type": "string",
                            "description": "Detailed invariant or architectural specification"
                        },
                        "version": {
                            "type": "string",
                            "description": "Version string or status"
                        }
                    },
                    "required": ["project_id", "entity_name", "entity_type", "definition"]
                }
            },
            {
                "name": "chronofact_memory_dossier",
                "description": "Retrieves the persistent Project Truth Dossier containing active tech stack, architectural invariants, and decisions from previous chat sessions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": {
                            "type": "string",
                            "description": "Identifier for the project"
                        },
                        "query": {
                            "type": "string",
                            "description": "Optional user prompt to relevance-gate memory retrieval and enforce the Zero-Pollution Guard (returns empty if unrelated)"
                        }
                    },
                    "required": ["project_id"]
                }
            },
            {
                "name": "chronofact_query",
                "description": "Full-cycle epistemic pipeline: checks temporal cutoff, retrieves grounded search, injects cross-session memory, and prepares verification context.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": {
                            "type": "string",
                            "description": "Project ID (e.g. 'chronofact' or current workspace name)"
                        },
                        "model_id": {
                            "type": "string",
                            "description": "Selected model identifier"
                        },
                        "query": {
                            "type": "string",
                            "description": "User prompt"
                        }
                    },
                    "required": ["project_id", "model_id", "query"]
                }
            },
            {
                "name": "chronofact_cost_optimize",
                "description": "Evaluates a prompt or task against available tools using TF-IDF routing, pruning irrelevant tool schemas to cut prompt tokens by 70-90% and generate prompt-cache alignment anchors.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The user prompt or task goal to evaluate for tool routing"
                        },
                        "top_k": {
                            "type": "integer",
                            "description": "Maximum number of relevant tools to retain (default: 4)"
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "chronofact_cost_metrics",
                "description": "Returns real-time token savings, prompt cache hit rate, and estimated USD cost saved across the session.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "gateway_find_tools",
                "description": "Searches across all connected upstream MCP servers and native tools using TF-IDF semantic relevance. Returns matched tools with full schemas, slashing prompt token bloat by 80%+.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Natural language description of what you want to accomplish"
                        },
                        "top_k": {
                            "type": "integer",
                            "description": "Number of tools to return (default: 5)"
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "gateway_call_tool",
                "description": "Executes a tool on a multiplexed upstream MCP server with inbound security sanitization, idempotent caching, and outbound anti-hallucination verification.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Exact name or fully-qualified name (server/tool) of the upstream tool"
                        },
                        "arguments": {
                            "type": "object",
                            "description": "Arguments to pass to the tool matching its inputSchema"
                        },
                        "verify_output": {
                            "type": "boolean",
                            "description": "Whether to perform deterministic lexical and invariant anti-hallucination claim audit on tool output (default: true)"
                        },
                        "passthrough": {
                            "type": "boolean",
                            "description": "When true, returns raw upstream MCP content without metadata wrapping and bypasses argument threat blocking for drop-in gateway compatibility (default: false)"
                        }
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "gateway_list_servers",
                "description": "Lists all connected upstream MCP servers, transport types (stdio/HTTP), tool counts, and connection health.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "chronofact_verify_code_invariants",
                "description": "Performs enterprise architectural and security invariant audits on source code, PR diffs, or manifests (detecting SQL injection, hardcoded secrets, layer/DAL leaks, deprecated libraries, and SSRF). Returns structured pass/block report.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Target relative or absolute file path being audited (e.g. 'src/services/billing.rs')"
                        },
                        "content": {
                            "type": "string",
                            "description": "The source code, diff, or configuration to evaluate against enterprise invariants"
                        },
                        "custom_rules": {
                            "type": "array",
                            "description": "Optional custom architectural invariant rules to enforce",
                            "items": { "type": "object" }
                        }
                    },
                    "required": ["file_path", "content"]
                }
            },
            {
                "name": "chronofact_attestation_generate",
                "description": "Generates a cryptographically signed Invariant Attestation certificate (SHA-256 HMAC) binding code content, temporal anchor, rules evaluated, and verification verdict for CI/CD gates.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_id": {
                            "type": "string",
                            "description": "Identifier of the project"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Path of the verified file or diff"
                        },
                        "content": {
                            "type": "string",
                            "description": "The exact audited content"
                        },
                        "temporal_anchor": {
                            "type": "string",
                            "description": "Active temporal anchor string or evaluation date"
                        }
                    },
                    "required": ["project_id", "file_path", "content"]
                }
            }
        ]
    })
}
