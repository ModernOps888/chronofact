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
