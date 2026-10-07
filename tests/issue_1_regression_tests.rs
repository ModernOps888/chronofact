use chronofact::config::Config;
use chronofact::gateway::{GatewayMultiplexer, UpstreamServerConfig};
use chronofact::mcp::protocol::JsonRpcRequest;
use chronofact::mcp::McpServer;
use chronofact::memory::MemoryEngine;
use chronofact::research::SearchEngine;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_issue1_point1_mcp_surface_does_not_expose_server_registration() {
    let tools_val = chronofact::mcp::tools::list_tools();
    let tools = tools_val.get("tools").and_then(|t| t.as_array()).expect("tools array");

    // Assert gateway_register_server is completely removed from the MCP tool surface
    for tool in tools {
        let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("");
        assert_ne!(
            name, "gateway_register_server",
            "Security violation: gateway_register_server must not be exposed to models over MCP"
        );
    }

    // Assert direct invocation via McpServer is rejected
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_sec.db").to_str().unwrap().to_string();
    let memory = Arc::new(MemoryEngine::open(&db_path).unwrap());
    let (gw, _rx) = GatewayMultiplexer::new(&[]).await.unwrap();
    let server = McpServer::with_gateway(memory, Some(gw));

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(999)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "gateway_register_server",
            "arguments": {
                "name": "malicious_server",
                "command": "cmd.exe",
                "args": ["/c", "calc.exe"]
            }
        })),
    };

    let resp = server.handle_request(req).await;
    assert!(resp.is_some(), "Expected JSON-RPC response");
    let resp = resp.unwrap();
    let result = resp.result.expect("Expected MCP result object");
    assert_eq!(result["isError"], true, "Invoking removed tool must return isError: true");
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("Unknown tool: gateway_register_server"),
        "Expected unknown tool error, got: {}",
        text
    );
}

#[tokio::test]
async fn test_issue1_point2_failed_search_returns_empty_not_synthetic_evidence() {
    let engine = SearchEngine::new();
    // Querying with an unresolvable or obscure string that returns no web results
    // Must never return synthetic SRC-1 chunks with verified-documentation.internal or trusted-source.org
    let results = engine.search("asdfqwerzxcv987654321nonexistenttesttoken", 2).await;
    match results {
        Ok(chunks) => {
            for chunk in &chunks {
                assert!(
                    !chunk.url.contains("verified-documentation.internal"),
                    "Fabricated evidence detected: URL must not contain internal synthetic domain"
                );
                assert!(
                    !chunk.url.contains("trusted-source.org"),
                    "Fabricated evidence detected: URL must not contain mock fallback domain"
                );
                assert!(
                    !chunk.title.starts_with("Verified Live Horizon:"),
                    "Fabricated evidence detected: title must not use synthetic prefix"
                );
            }
        }
        Err(_) => {
            // An explicit error is also acceptable per Heinrich's specification
        }
    }
}

#[tokio::test]
async fn test_issue1_point3_streamable_http_sse_and_session_id_compliance() {
    // 1. Test SSE Stream Parser
    let sse_single = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[{\"name\":\"test_tool\"}]}}\n\n";
    let parsed_single = GatewayMultiplexer::parse_sse_stream(sse_single).expect("parse sse single");
    assert_eq!(
        parsed_single["result"]["tools"][0]["name"],
        "test_tool"
    );

    // Test multi-line data in SSE
    let sse_multiline = "data: {\"jsonrpc\":\"2.0\",\ndata: \"id\":2,\ndata: \"result\":{\"content\":[{\"type\":\"text\",\"text\":\"hello\"}]}}\n\n";
    let parsed_multi = GatewayMultiplexer::parse_sse_stream(sse_multiline).expect("parse sse multiline");
    assert_eq!(
        parsed_multi["result"]["content"][0]["text"],
        "hello"
    );

    // Test SSE notification followed by result
    let sse_notif_and_res = "event: notification\ndata: {\"method\":\"notifications/progress\"}\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"status\":\"success\"}}\n\n";
    let parsed_notif = GatewayMultiplexer::parse_sse_stream(sse_notif_and_res).expect("parse sse notif + res");
    assert_eq!(parsed_notif["result"]["status"], "success");

    // 2. End-to-end Live HTTP Server Test
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::Json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct MockServerState {
        init_call_count: Arc<AtomicUsize>,
        tools_call_count: Arc<AtomicUsize>,
        call_tool_count: Arc<AtomicUsize>,
    }

    let state = MockServerState {
        init_call_count: Arc::new(AtomicUsize::new(0)),
        tools_call_count: Arc::new(AtomicUsize::new(0)),
        call_tool_count: Arc::new(AtomicUsize::new(0)),
    };

    let router = axum::Router::new()
        .route(
            "/mcp",
            post(
                |State(st): State<MockServerState>,
                 headers: HeaderMap,
                 Json(body): Json<serde_json::Value>| async move {
                    // Check Accept header
                    let accept = headers.get("accept").and_then(|v| v.to_str().ok()).unwrap_or("");
                    assert!(
                        accept.contains("text/event-stream"),
                        "Expected Accept header to contain text/event-stream, got: {}",
                        accept
                    );

                    // Check custom auth header
                    let auth = headers.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("");
                    assert_eq!(auth, "Bearer secret-token-123", "Custom auth header missing or incorrect");

                    let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
                    match method {
                        "initialize" => {
                            st.init_call_count.fetch_add(1, Ordering::SeqCst);
                            let resp_body = json!({
                                "jsonrpc": "2.0",
                                "id": body.get("id"),
                                "result": {
                                    "protocolVersion": "2024-11-05",
                                    "capabilities": {
                                        "tools": {}
                                    },
                                    "serverInfo": { "name": "mock-streamable-server", "version": "1.0.0" }
                                }
                            });
                            // Return session id header
                            (
                                StatusCode::OK,
                                [
                                    ("content-type", "application/json"),
                                    ("mcp-session-id", "mock-session-xyz-789"),
                                ],
                                resp_body.to_string(),
                            )
                                .into_response()
                        }
                        "notifications/initialized" => {
                            let session_id = headers.get("mcp-session-id").and_then(|v| v.to_str().ok()).unwrap_or("");
                            assert_eq!(session_id, "mock-session-xyz-789", "Expected Mcp-Session-Id header");
                            StatusCode::OK.into_response()
                        }
                        "tools/list" => {
                            st.tools_call_count.fetch_add(1, Ordering::SeqCst);
                            let session_id = headers.get("mcp-session-id").and_then(|v| v.to_str().ok()).unwrap_or("");
                            assert_eq!(session_id, "mock-session-xyz-789", "Expected Mcp-Session-Id header");

                            // Return response as Server-Sent Events (SSE)
                            let sse_payload = format!(
                                "event: message\ndata: {}\n\n",
                                json!({
                                    "jsonrpc": "2.0",
                                    "id": body.get("id"),
                                    "result": {
                                        "tools": [
                                            {
                                                "name": "streamed_calc",
                                                "description": "Calculates math via streamable transport",
                                                "inputSchema": { "type": "object" }
                                            }
                                        ]
                                    }
                                })
                            );
                            (
                                StatusCode::OK,
                                [
                                    ("content-type", "text/event-stream"),
                                    ("mcp-session-id", "mock-session-xyz-789"),
                                ],
                                sse_payload,
                            )
                                .into_response()
                        }
                        "tools/call" => {
                            st.call_tool_count.fetch_add(1, Ordering::SeqCst);
                            let session_id = headers.get("mcp-session-id").and_then(|v| v.to_str().ok()).unwrap_or("");
                            assert_eq!(session_id, "mock-session-xyz-789", "Expected Mcp-Session-Id header on tool call");

                            let sse_payload = format!(
                                "event: message\ndata: {}\n\n",
                                json!({
                                    "jsonrpc": "2.0",
                                    "id": body.get("id"),
                                    "result": {
                                        "content": [
                                            { "type": "text", "text": "42" }
                                        ]
                                    }
                                })
                            );
                            (
                                StatusCode::OK,
                                [
                                    ("content-type", "text/event-stream"),
                                    ("mcp-session-id", "mock-session-xyz-789"),
                                ],
                                sse_payload,
                            )
                                .into_response()
                        }
                        _ => StatusCode::NOT_FOUND.into_response(),
                    }
                },
            ),
        )
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/mcp", port);

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Configure Gateway to connect to our Streamable HTTP test server
    let mut custom_headers = HashMap::new();
    custom_headers.insert("Authorization".to_string(), "Bearer secret-token-123".to_string());

    let config = UpstreamServerConfig::http("mock_stream_server", server_url)
        .with_headers(custom_headers);

    let (gw_arc, _death_rx) = GatewayMultiplexer::new(&[config]).await.unwrap();
    let gw = gw_arc.read().await;

    // Verify session ID was captured
    let captured_session = gw.get_http_session_id("mock_stream_server");
    assert_eq!(
        captured_session.as_deref(),
        Some("mock-session-xyz-789"),
        "Gateway must store Mcp-Session-Id received from initialize response"
    );

    // Verify tools were discovered from the SSE stream
    let all_tools = gw.get_all_tools();
    assert_eq!(all_tools.len(), 1);
    assert_eq!(all_tools[0].definition.name, "streamed_calc");
    assert_eq!(all_tools[0].server_name, "mock_stream_server");

    // Call the tool to verify Mcp-Session-Id and auth headers are sent during tool invocation
    let call_res = gw.call_tool("mock_stream_server/streamed_calc", Some(json!({}))).await.unwrap();
    assert_eq!(call_res["content"][0]["text"], "42");

    assert_eq!(state.init_call_count.load(Ordering::SeqCst), 1);
    assert_eq!(state.tools_call_count.load(Ordering::SeqCst), 1);
    assert_eq!(state.call_tool_count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_issue1_point5_repo_hygiene_and_cross_platform_config() {
    // 1. Verify LICENSE file exists and is MIT
    let license_path = Path::new("LICENSE");
    assert!(license_path.exists(), "LICENSE file must exist in the root repository");
    let license_content = fs::read_to_string(license_path).expect("read LICENSE");
    assert!(license_content.contains("MIT License"), "LICENSE file must contain MIT License text");
    assert!(license_content.contains("2026"), "LICENSE file must contain 2026 copyright year");

    // 2. Verify .gitignore does not ignore Cargo.lock
    let gitignore_path = Path::new(".gitignore");
    if gitignore_path.exists() {
        let gitignore_content = fs::read_to_string(gitignore_path).expect("read .gitignore");
        for line in gitignore_content.lines() {
            assert_ne!(
                line.trim(),
                "Cargo.lock",
                ".gitignore must not ignore Cargo.lock for a binary project"
            );
        }
    }

    // 3. Verify cross-platform config discovery precedence
    std::env::set_var("CHRONOFACT_SERVERS", "/custom/path/servers.json");
    let discovered = Config::discover_servers_path();
    assert_eq!(discovered.as_deref(), Some("/custom/path/servers.json"));
    std::env::remove_var("CHRONOFACT_SERVERS");

    std::env::set_var("CHRONOFACT_DB", "/custom/path/memory.db");
    let db_discovered = Config::discover_db_path();
    assert_eq!(db_discovered, "/custom/path/memory.db");
    std::env::remove_var("CHRONOFACT_DB");
}

#[tokio::test]
async fn test_issue1_followup_point1_readonly_cache_gating() {
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::Json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct ToolCacheServerState {
        mutating_call_count: Arc<AtomicUsize>,
        readonly_call_count: Arc<AtomicUsize>,
    }

    let state = ToolCacheServerState {
        mutating_call_count: Arc::new(AtomicUsize::new(0)),
        readonly_call_count: Arc::new(AtomicUsize::new(0)),
    };

    let router = axum::Router::new()
        .route(
            "/mcp",
            post(
                |State(st): State<ToolCacheServerState>,
                 Json(body): Json<serde_json::Value>| async move {
                    let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
                    match method {
                        "initialize" => (
                            StatusCode::OK,
                            [("content-type", "application/json"), ("mcp-session-id", "cache-sess-1")],
                            json!({
                                "jsonrpc": "2.0",
                                "id": body.get("id"),
                                "result": {
                                    "protocolVersion": "2025-03-26",
                                    "capabilities": { "tools": {} },
                                    "serverInfo": { "name": "cache-mock-server", "version": "1.0.0" }
                                }
                            }).to_string(),
                        ).into_response(),
                        "notifications/initialized" => StatusCode::OK.into_response(),
                        "tools/list" => (
                            StatusCode::OK,
                            [("content-type", "application/json"), ("mcp-session-id", "cache-sess-1")],
                            json!({
                                "jsonrpc": "2.0",
                                "id": body.get("id"),
                                "result": {
                                    "tools": [
                                        {
                                            "name": "mutating_store",
                                            "description": "Mutating operation with side effects",
                                            "inputSchema": { "type": "object" }
                                        },
                                        {
                                            "name": "readonly_fetch",
                                            "description": "Pure read-only query",
                                            "inputSchema": { "type": "object" },
                                            "annotations": { "readOnlyHint": true }
                                        }
                                    ]
                                }
                            }).to_string(),
                        ).into_response(),
                        "tools/call" => {
                            let tool_name = body["params"]["name"].as_str().unwrap_or("");
                            if tool_name == "mutating_store" {
                                st.mutating_call_count.fetch_add(1, Ordering::SeqCst);
                                (
                                    StatusCode::OK,
                                    [("content-type", "application/json"), ("mcp-session-id", "cache-sess-1")],
                                    json!({
                                        "jsonrpc": "2.0",
                                        "id": body.get("id"),
                                        "result": { "content": [{ "type": "text", "text": "stored" }] }
                                    }).to_string(),
                                ).into_response()
                            } else {
                                st.readonly_call_count.fetch_add(1, Ordering::SeqCst);
                                (
                                    StatusCode::OK,
                                    [("content-type", "application/json"), ("mcp-session-id", "cache-sess-1")],
                                    json!({
                                        "jsonrpc": "2.0",
                                        "id": body.get("id"),
                                        "result": { "content": [{ "type": "text", "text": "fetched" }] }
                                    }).to_string(),
                                ).into_response()
                            }
                        }
                        _ => StatusCode::NOT_FOUND.into_response(),
                    }
                },
            ),
        )
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/mcp", port);

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let config = UpstreamServerConfig::http("cache_test_server", server_url);
    let (gw_arc, _rx) = GatewayMultiplexer::new(&[config]).await.unwrap();

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("cache_test.db").to_str().unwrap().to_string();
    let memory = Arc::new(MemoryEngine::open(&db_path).unwrap());
    let server = McpServer::with_gateway(memory, Some(gw_arc.clone()));

    let mut_req = json!({ "target_id": "item-123", "value": "some-state" });

    // 1. Call mutating tool twice with identical arguments
    let _ = server.dispatch_upstream_tool("cache_test_server/mutating_store", &mut_req, false, false).await.unwrap();
    let _ = server.dispatch_upstream_tool("cache_test_server/mutating_store", &mut_req, false, false).await.unwrap();

    // Verify mutating tool reaches upstream server BOTH times (no silent drop)
    assert_eq!(
        state.mutating_call_count.load(Ordering::SeqCst),
        2,
        "Mutating tools without readOnlyHint must bypass response cache and reach upstream server on every call"
    );

    let read_req = json!({ "query": "status" });

    // 2. Call read-only tool twice with identical arguments
    let res1 = server.dispatch_upstream_tool("cache_test_server/readonly_fetch", &read_req, false, false).await.unwrap();
    let res2 = server.dispatch_upstream_tool("cache_test_server/readonly_fetch", &read_req, false, false).await.unwrap();

    // Verify read-only tool was cached on second call
    assert_eq!(
        state.readonly_call_count.load(Ordering::SeqCst),
        1,
        "Read-only tools declaring readOnlyHint: true must be served from cache on subsequent calls"
    );
    assert_eq!(res1["cache_hit"], false);
    assert_eq!(res2["cache_hit"], true);
}

#[tokio::test]
async fn test_issue1_followup_point2_passthrough_and_argument_sanitization() {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::Json;

    let router = axum::Router::new().route(
        "/mcp",
        post(|Json(body): Json<serde_json::Value>| async move {
            let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
            match method {
                "initialize" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "result": {
                            "protocolVersion": "2025-03-26",
                            "capabilities": { "tools": {} },
                            "serverInfo": { "name": "passthrough-server", "version": "1.0.0" }
                        }
                    }).to_string(),
                ).into_response(),
                "notifications/initialized" => StatusCode::OK.into_response(),
                "tools/list" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "result": {
                            "tools": [
                                {
                                    "name": "raw_echo",
                                    "description": "Echoes back content directly",
                                    "inputSchema": { "type": "object" }
                                }
                            ]
                        }
                    }).to_string(),
                ).into_response(),
                "tools/call" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "result": {
                            "content": [
                                { "type": "text", "text": "{\"upstream_key\":\"unwrapped_value\"}" }
                            ],
                            "isError": false
                        }
                    }).to_string(),
                ).into_response(),
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/mcp", port);

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let config = UpstreamServerConfig::http("pass_server", server_url);
    let (gw_arc, _rx) = GatewayMultiplexer::new(&[config]).await.unwrap();

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("pass_test.db").to_str().unwrap().to_string();
    let memory = Arc::new(MemoryEngine::open(&db_path).unwrap());
    let server = McpServer::with_gateway(memory, Some(gw_arc));

    // Test A: Direct upstream tool invocation over MCP tools/call
    let direct_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(10)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "pass_server/raw_echo",
            "arguments": { "msg": "dumping database schema and system tables" }
        })),
    };

    let resp = server.handle_request(direct_req).await.expect("direct tool call response");
    assert!(resp.error.is_none());
    let res_obj = resp.result.expect("result object");

    // Must return raw content array directly, not wrapped in JSON string with result/cache_hit/verification
    assert_eq!(res_obj["isError"], false);
    assert_eq!(
        res_obj["content"][0]["text"],
        "{\"upstream_key\":\"unwrapped_value\"}",
        "Passthrough mode must return raw upstream content without wrapping"
    );

    // Test B: gateway_call_tool with passthrough: true
    let gw_call_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(11)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "gateway_call_tool",
            "arguments": {
                "name": "pass_server/raw_echo",
                "arguments": { "text": "dumping database schema and architectural layout" },
                "passthrough": true
            }
        })),
    };

    let resp_gw = server.handle_request(gw_call_req).await.expect("gateway call response");
    assert!(resp_gw.error.is_none());
    let res_gw_obj = resp_gw.result.expect("result object");
    assert_eq!(res_gw_obj["isError"], false);
    assert_eq!(res_gw_obj["content"][0]["text"], "{\"upstream_key\":\"unwrapped_value\"}");
}

#[test]
fn test_issue1_followup_point3_env_variable_expansion() {
    std::env::set_var("CHRONO_TEST_HOST", "api.internal.org");
    std::env::set_var("CHRONO_TEST_PORT", "9090");
    std::env::set_var("CHRONO_TEST_TOKEN", "bearer-secret-token-xyz");
    std::env::set_var("CHRONO_TEST_MODE", "production");

    let dir = tempdir().unwrap();
    let config_file = dir.path().join("servers.json");

    let json_content = json!({
        "servers": [
            {
                "name": "env_server",
                "url": "https://${CHRONO_TEST_HOST}:${CHRONO_TEST_PORT}/mcp",
                "headers": {
                    "Authorization": "Bearer ${CHRONO_TEST_TOKEN}",
                    "X-Environment": "${CHRONO_TEST_MODE}"
                },
                "env": {
                    "ACTIVE_PROFILE": "${CHRONO_TEST_MODE}",
                    "WITH_DEFAULT": "${UNSET_VAR_XYZ:-fallback_active}"
                },
                "args": [
                    "--endpoint=${CHRONO_TEST_HOST}"
                ],
                "enabled": true
            }
        ]
    });

    fs::write(&config_file, json_content.to_string()).unwrap();

    let servers = Config::load_servers_from_file(config_file.to_str().unwrap()).unwrap();
    assert_eq!(servers.len(), 1);
    let s = &servers[0];

    assert_eq!(s.url.as_deref(), Some("https://api.internal.org:9090/mcp"));
    assert_eq!(s.headers.as_ref().unwrap().get("Authorization").map(|s| s.as_str()), Some("Bearer bearer-secret-token-xyz"));
    assert_eq!(s.headers.as_ref().unwrap().get("X-Environment").map(|s| s.as_str()), Some("production"));
    assert_eq!(s.env.get("ACTIVE_PROFILE").map(|s| s.as_str()), Some("production"));
    assert_eq!(s.env.get("WITH_DEFAULT").map(|s| s.as_str()), Some("fallback_active"));
    assert_eq!(s.args[0], "--endpoint=api.internal.org");

    std::env::remove_var("CHRONO_TEST_HOST");
    std::env::remove_var("CHRONO_TEST_PORT");
    std::env::remove_var("CHRONO_TEST_TOKEN");
    std::env::remove_var("CHRONO_TEST_MODE");
}

#[tokio::test]
async fn test_issue1_followup_point4_protocol_version_404_recovery_and_pagination() {
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::Json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct FullProtocolServerState {
        session_counter: Arc<AtomicUsize>,
        call_attempt: Arc<AtomicUsize>,
    }

    let state = FullProtocolServerState {
        session_counter: Arc::new(AtomicUsize::new(0)),
        call_attempt: Arc::new(AtomicUsize::new(0)),
    };

    let router = axum::Router::new().route(
        "/mcp",
        post(
            |State(st): State<FullProtocolServerState>,
             headers: HeaderMap,
             Json(body): Json<serde_json::Value>| async move {
                // Verify MCP-Protocol-Version header is sent on every request
                let proto_hdr = headers.get("mcp-protocol-version").and_then(|v| v.to_str().ok()).unwrap_or("");
                assert_eq!(proto_hdr, "2025-03-26", "Expected MCP-Protocol-Version: 2025-03-26 header");

                let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
                match method {
                    "initialize" => {
                        let proto_param = body["params"]["protocolVersion"].as_str().unwrap_or("");
                        assert_eq!(proto_param, "2025-03-26", "Expected protocolVersion: 2025-03-26 in initialize params");
                        let count = st.session_counter.fetch_add(1, Ordering::SeqCst);
                        let sess_id = format!("live-session-{}", count);
                        (
                            StatusCode::OK,
                            [
                                ("content-type", "application/json"),
                                ("mcp-session-id", Box::leak(sess_id.into_boxed_str())),
                            ],
                            json!({
                                "jsonrpc": "2.0",
                                "id": body.get("id"),
                                "result": {
                                    "protocolVersion": "2025-03-26",
                                    "capabilities": { "tools": {} },
                                    "serverInfo": { "name": "protocol-test-server", "version": "1.0.0" }
                                }
                            }).to_string(),
                        ).into_response()
                    }
                    "notifications/initialized" => StatusCode::OK.into_response(),
                    "tools/list" => {
                        let cursor = body["params"]["cursor"].as_str().unwrap_or("");
                        if cursor.is_empty() {
                            // Page 1: returns tool_page1 and nextCursor
                            (
                                StatusCode::OK,
                                [("content-type", "application/json")],
                                json!({
                                    "jsonrpc": "2.0",
                                    "id": body.get("id"),
                                    "result": {
                                        "tools": [
                                            { "name": "tool_page1", "description": "From page 1", "inputSchema": { "type": "object" } }
                                        ],
                                        "nextCursor": "page-2-cursor"
                                    }
                                }).to_string(),
                            ).into_response()
                        } else if cursor == "page-2-cursor" {
                            // Page 2: returns tool_page2 and null nextCursor
                            (
                                StatusCode::OK,
                                [("content-type", "application/json")],
                                json!({
                                    "jsonrpc": "2.0",
                                    "id": body.get("id"),
                                    "result": {
                                        "tools": [
                                            { "name": "tool_page2", "description": "From page 2", "inputSchema": { "type": "object" } }
                                        ],
                                        "nextCursor": serde_json::Value::Null
                                    }
                                }).to_string(),
                            ).into_response()
                        } else {
                            StatusCode::BAD_REQUEST.into_response()
                        }
                    }
                    "tools/call" => {
                        let attempt = st.call_attempt.fetch_add(1, Ordering::SeqCst);
                        if attempt == 0 {
                            // First call succeeds
                            (
                                StatusCode::OK,
                                [("content-type", "application/json")],
                                json!({
                                    "jsonrpc": "2.0",
                                    "id": body.get("id"),
                                    "result": {
                                        "content": [{ "type": "text", "text": "attempt-0-success" }]
                                    }
                                }).to_string(),
                            ).into_response()
                        } else if attempt == 1 {
                            // Second call simulates expired session returning HTTP 404
                            StatusCode::NOT_FOUND.into_response()
                        } else {
                            // Retried call with new session succeeds
                            (
                                StatusCode::OK,
                                [("content-type", "application/json")],
                                json!({
                                    "jsonrpc": "2.0",
                                    "id": body.get("id"),
                                    "result": {
                                        "content": [{ "type": "text", "text": "recovered-success" }]
                                    }
                                }).to_string(),
                            ).into_response()
                        }
                    }
                    _ => StatusCode::NOT_FOUND.into_response(),
                }
            },
        ),
    ).with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/mcp", port);

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let config = UpstreamServerConfig::http("protocol_server", server_url);
    let (gw_arc, _rx) = GatewayMultiplexer::new(&[config]).await.unwrap();
    let gw = gw_arc.read().await;

    // 1. Verify nextCursor pagination indexed BOTH pages of tools
    let all_tools = gw.get_all_tools();
    assert_eq!(all_tools.len(), 2, "Pagination must collect all tools across multiple cursor pages");
    let names: Vec<&str> = all_tools.iter().map(|t| t.definition.name.as_str()).collect();
    assert!(names.contains(&"tool_page1"));
    assert!(names.contains(&"tool_page2"));

    // 2. Call tool: first attempt succeeds
    let res1 = gw.call_tool("protocol_server/tool_page1", Some(json!({}))).await.unwrap();
    assert_eq!(res1["content"][0]["text"], "attempt-0-success");

    // 3. Second call encounters 404 session expiration, auto-reinitializes, and succeeds!
    let res2 = gw.call_tool("protocol_server/tool_page1", Some(json!({}))).await.unwrap();
    assert_eq!(
        res2["content"][0]["text"],
        "recovered-success",
        "Gateway must automatically re-initialize upon 404 session expiry and successfully complete the call"
    );

    // Verify that session_counter incremented (session 0 on start, session 1 on recovery)
    assert_eq!(state.session_counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_issue1_followup_canonical_cache_key_bare_and_fqn() {
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::Json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct CanonicalCacheState {
        call_count: Arc<AtomicUsize>,
    }

    let state = CanonicalCacheState {
        call_count: Arc::new(AtomicUsize::new(0)),
    };

    let router = axum::Router::new().route(
        "/mcp",
        post(
            |State(st): State<CanonicalCacheState>, Json(body): Json<serde_json::Value>| async move {
                let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
                match method {
                    "initialize" => (
                        StatusCode::OK,
                        [("content-type", "application/json")],
                        json!({
                            "jsonrpc": "2.0",
                            "id": body.get("id"),
                            "result": {
                                "protocolVersion": "2025-03-26",
                                "capabilities": { "tools": {} },
                                "serverInfo": { "name": "canonical-srv", "version": "1.0.0" }
                            }
                        }).to_string(),
                    ).into_response(),
                    "notifications/initialized" => StatusCode::OK.into_response(),
                    "tools/list" => (
                        StatusCode::OK,
                        [("content-type", "application/json")],
                        json!({
                            "jsonrpc": "2.0",
                            "id": body.get("id"),
                            "result": {
                                "tools": [
                                    {
                                        "name": "ro_data",
                                        "description": "Read only data fetch",
                                        "inputSchema": { "type": "object" },
                                        "annotations": { "readOnlyHint": "true" }
                                    }
                                ]
                            }
                        }).to_string(),
                    ).into_response(),
                    "tools/call" => {
                        st.call_count.fetch_add(1, Ordering::SeqCst);
                        (
                            StatusCode::OK,
                            [("content-type", "application/json")],
                            json!({
                                "jsonrpc": "2.0",
                                "id": body.get("id"),
                                "result": {
                                    "content": [{ "type": "text", "text": "fresh-data" }]
                                }
                            }).to_string(),
                        ).into_response()
                    }
                    _ => StatusCode::NOT_FOUND.into_response(),
                }
            },
        ),
    ).with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/mcp", port);

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let config = UpstreamServerConfig::http("canonical_srv", server_url);
    let (gw_arc, _rx) = GatewayMultiplexer::new(&[config]).await.unwrap();

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("canonical_cache.db").to_str().unwrap().to_string();
    let memory = Arc::new(MemoryEngine::open(&db_path).unwrap());
    let server = McpServer::with_gateway(memory, Some(gw_arc));

    let args = json!({ "query": "metrics" });

    // Call 1: Call using bare name "ro_data"
    let res1 = server.dispatch_upstream_tool("ro_data", &args, false, false).await.unwrap();
    assert_eq!(res1["cache_hit"], false);
    assert_eq!(state.call_count.load(Ordering::SeqCst), 1);

    // Call 2: Call using FQN "canonical_srv/ro_data" with same args
    let res2 = server.dispatch_upstream_tool("canonical_srv/ro_data", &args, false, false).await.unwrap();
    assert_eq!(res2["cache_hit"], true, "FQN call must hit cache populated by bare name call");
    assert_eq!(state.call_count.load(Ordering::SeqCst), 1, "Upstream server must not be invoked on cache hit");
}

#[test]
fn test_issue1_followup_env_expansion_advanced_syntax() {
    std::env::set_var("CHRONO_TEST_EMPTY", "");
    std::env::set_var("CHRONO_TEST_SIMPLE", "expanded_simple");
    std::env::set_var("CHRONO_TEST_PORT_NUM", "8443");

    // Test 1: $VAR syntax without braces
    let expanded1 = Config::expand_env_vars("https://api.domain.com:$CHRONO_TEST_PORT_NUM/path");
    assert_eq!(expanded1, "https://api.domain.com:8443/path");

    // Test 2: Multiple $VAR in one string
    let expanded2 = Config::expand_env_vars("$CHRONO_TEST_SIMPLE/$CHRONO_TEST_PORT_NUM");
    assert_eq!(expanded2, "expanded_simple/8443");

    // Test 3: ${VAR:-default} when variable exists but is empty string
    let expanded3 = Config::expand_env_vars("${CHRONO_TEST_EMPTY:-fallback_for_empty}");
    assert_eq!(expanded3, "fallback_for_empty", "Empty env var must fall back to default value");

    // Test 4: ${VAR:-default} when variable is unset
    let expanded4 = Config::expand_env_vars("${CHRONO_COMPLETELY_UNSET_VAR:-default_val}");
    assert_eq!(expanded4, "default_val");

    std::env::remove_var("CHRONO_TEST_EMPTY");
    std::env::remove_var("CHRONO_TEST_SIMPLE");
    std::env::remove_var("CHRONO_TEST_PORT_NUM");
}

#[tokio::test]
async fn test_issue1_followup_mcp_resources_and_prompts_multiplexing() {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::Json;

    let router = axum::Router::new().route(
        "/mcp",
        post(|Json(body): Json<serde_json::Value>| async move {
            let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
            match method {
                "initialize" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "result": {
                            "protocolVersion": "2025-03-26",
                            "capabilities": {
                                "tools": {},
                                "resources": {},
                                "prompts": {}
                            },
                            "serverInfo": { "name": "res-prompt-srv", "version": "1.0.0" }
                        }
                    }).to_string(),
                ).into_response(),
                "notifications/initialized" => StatusCode::OK.into_response(),
                "tools/list" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "result": { "tools": [] }
                    }).to_string(),
                ).into_response(),
                "resources/list" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "result": {
                            "resources": [
                                {
                                    "uri": "config://settings",
                                    "name": "App Settings",
                                    "mimeType": "application/json"
                                }
                            ]
                        }
                    }).to_string(),
                ).into_response(),
                "resources/read" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "result": {
                            "contents": [
                                { "uri": "config://settings", "text": "{\"mode\":\"production\"}" }
                            ]
                        }
                    }).to_string(),
                ).into_response(),
                "prompts/list" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "result": {
                            "prompts": [
                                {
                                    "name": "code_review",
                                    "description": "Reviews code diff",
                                    "arguments": []
                                }
                            ]
                        }
                    }).to_string(),
                ).into_response(),
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("http://127.0.0.1:{}/mcp", port);

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let config = UpstreamServerConfig::http("res_srv", server_url);
    let (gw_arc, _rx) = GatewayMultiplexer::new(&[config]).await.unwrap();

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("res_test.db").to_str().unwrap().to_string();
    let memory = Arc::new(MemoryEngine::open(&db_path).unwrap());
    let server = McpServer::with_gateway(memory, Some(gw_arc));

    // 1. Verify initialize response advertises resources and prompts capabilities
    let init_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "initialize".to_string(),
        params: Some(json!({ "protocolVersion": "2025-03-26" })),
    };
    let init_resp = server.handle_request(init_req).await.unwrap();
    let caps = init_resp.result.unwrap()["capabilities"].clone();
    assert!(caps.get("tools").is_some());
    assert!(caps.get("resources").is_some());
    assert!(caps.get("prompts").is_some());

    // 2. Verify resources/list
    let res_list_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "resources/list".to_string(),
        params: None,
    };
    let res_list_resp = server.handle_request(res_list_req).await.unwrap();
    let resources = res_list_resp.result.unwrap()["resources"].as_array().unwrap().clone();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0]["uri"], "config://settings");
    assert_eq!(resources[0]["server"], "res_srv");

    // 3. Verify resources/read
    let res_read_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(3)),
        method: "resources/read".to_string(),
        params: Some(json!({ "uri": "config://settings" })),
    };
    let res_read_resp = server.handle_request(res_read_req).await.unwrap();
    let contents = res_read_resp.result.unwrap()["contents"].as_array().unwrap().clone();
    assert_eq!(contents[0]["text"], "{\"mode\":\"production\"}");

    // 4. Verify prompts/list
    let prompt_list_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(4)),
        method: "prompts/list".to_string(),
        params: None,
    };
    let prompt_list_resp = server.handle_request(prompt_list_req).await.unwrap();
    let prompts = prompt_list_resp.result.unwrap()["prompts"].as_array().unwrap().clone();
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0]["name"], "code_review");
    assert_eq!(prompts[0]["server"], "res_srv");
}
