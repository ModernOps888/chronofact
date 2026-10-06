use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::stdio::{DeathReceiver, DeathSender, StdioConnection};
use super::types::{
    PromptDefinition, RegisteredPrompt, RegisteredResource, RegisteredTool, ResourceDefinition,
    ToolDefinition, UpstreamServerConfig, UpstreamServerStatus,
};
use crate::cost::ToolCandidate;

/// Represents an upstream MCP server and its discovered capabilities
#[derive(Debug, Clone)]
pub struct UpstreamServer {
    pub name: String,
    pub config: UpstreamServerConfig,
    pub tools: Vec<ToolDefinition>,
    pub resources: Vec<ResourceDefinition>,
    pub prompts: Vec<PromptDefinition>,
    pub connected: bool,
}

#[derive(Clone)]
pub enum ToolExecutionTarget {
    Http {
        server_name: String,
        url: String,
        config: UpstreamServerConfig,
        bare_name: String,
        arguments: Option<serde_json::Value>,
        client: reqwest::Client,
        session_id: Option<String>,
    },
    Stdio {
        conn: Arc<StdioConnection>,
        bare_name: String,
        arguments: Option<serde_json::Value>,
    },
}

impl ToolExecutionTarget {
    pub async fn execute(self) -> anyhow::Result<serde_json::Value> {
        match self {
            ToolExecutionTarget::Http {
                server_name: _,
                url,
                config,
                bare_name,
                arguments,
                client,
                session_id,
            } => {
                let req_body = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/call",
                    "params": {
                        "name": bare_name,
                        "arguments": arguments,
                    }
                });

                let mut builder = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json, text/event-stream");

                if let Some(ref sid) = session_id {
                    builder = builder.header("Mcp-Session-Id", sid);
                }

                if let Some(ref headers) = config.headers {
                    for (k, v) in headers {
                        builder = builder.header(k, v);
                    }
                }

                let resp = builder.json(&req_body).send().await?;
                let status = resp.status();
                let content_type = resp
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let body_text = resp.text().await?;

                if !status.is_success() {
                    return Err(anyhow::anyhow!(
                        "Upstream HTTP server returned status {}: {}",
                        status,
                        body_text
                    ));
                }

                let body = GatewayMultiplexer::parse_mcp_response(content_type.as_deref(), &body_text)?;
                if let Some(res) = body.get("result") {
                    Ok(res.clone())
                } else if let Some(err) = body.get("error") {
                    Err(anyhow::anyhow!("Upstream error: {}", err))
                } else {
                    Err(anyhow::anyhow!("Invalid response from upstream HTTP server: {}", body))
                }
            }
            ToolExecutionTarget::Stdio {
                conn,
                bare_name,
                arguments,
            } => {
                conn.send_request(
                    "tools/call",
                    serde_json::json!({
                        "name": bare_name,
                        "arguments": arguments,
                    }),
                )
                .await
            }
        }
    }
}

/// The GatewayMultiplexer manages connections, indexing, and routing across multiple MCP servers
pub struct GatewayMultiplexer {
    servers: HashMap<String, UpstreamServer>,
    stdio_connections: HashMap<String, Arc<StdioConnection>>,
    /// tool_name or server/tool_name -> server_name
    tool_index: HashMap<String, String>,
    /// resource_uri or server/resource_uri -> server_name
    resource_index: HashMap<String, String>,
    /// prompt_name or server/prompt_name -> server_name
    prompt_index: HashMap<String, String>,
    all_tools: Vec<RegisteredTool>,
    all_resources: Vec<RegisteredResource>,
    all_prompts: Vec<RegisteredPrompt>,
    death_tx: DeathSender,
    http_client: reqwest::Client,
    http_sessions: std::sync::RwLock<HashMap<String, String>>,
}

impl GatewayMultiplexer {
    /// Create a new gateway multiplexer and initialize configured upstream servers
    pub async fn new(
        server_configs: &[UpstreamServerConfig],
    ) -> anyhow::Result<(Arc<RwLock<Self>>, DeathReceiver)> {
        let (death_tx, death_rx) = tokio::sync::mpsc::unbounded_channel();
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(64)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        let mut multiplexer = Self {
            servers: HashMap::new(),
            stdio_connections: HashMap::new(),
            tool_index: HashMap::new(),
            resource_index: HashMap::new(),
            prompt_index: HashMap::new(),
            all_tools: Vec::new(),
            all_resources: Vec::new(),
            all_prompts: Vec::new(),
            death_tx,
            http_client,
            http_sessions: std::sync::RwLock::new(HashMap::new()),
        };

        for config in server_configs {
            if !config.enabled {
                info!("⏭️ Skipping disabled upstream server: {}", config.name);
                continue;
            }
            multiplexer.register_and_connect(config.clone()).await;
        }

        let locked = Arc::new(RwLock::new(multiplexer));
        Ok((locked, death_rx))
    }

    /// Register and connect to a single upstream server dynamically
    pub async fn register_and_connect(&mut self, config: UpstreamServerConfig) {
        let name = config.name.clone();

        let (tools, resources, prompts, connected) = if config.url.is_some() {
            // HTTP transport
            self.discover_http(&config).await
        } else if config.command.is_some() {
            // Stdio transport
            let req_timeout = Duration::from_secs(config.request_timeout_secs.unwrap_or(30));
            let handshake_timeout = Duration::from_secs(config.handshake_timeout_secs.unwrap_or(15));
            self.discover_stdio(&config, req_timeout, handshake_timeout).await
        } else {
            warn!("Server '{}' lacks both 'command' and 'url' configuration", name);
            (Vec::new(), Vec::new(), Vec::new(), false)
        };

        if connected {
            info!(
                "📡 Gateway connected to '{}': {} tools, {} resources, {} prompts",
                name,
                tools.len(),
                resources.len(),
                prompts.len()
            );
        } else {
            warn!("⚠️ Gateway failed connecting to '{}' — registered in disconnected state", name);
        }

        // Index tools with collision disambiguation (MCPlex FQN pattern)
        for tool in &tools {
            let registered = RegisteredTool::new(tool.clone(), &name);
            self.tool_index.insert(registered.fqn.clone(), name.clone());

            if let Some(existing_server) = self.tool_index.get(&tool.name) {
                if existing_server != &name {
                    warn!(
                        "⚠️ Tool collision: '{}' registered by both '{}' and '{}'. Ambiguous bare name removed — use FQN 'server/tool'.",
                        tool.name, existing_server, name
                    );
                    self.tool_index.remove(&tool.name);
                }
            } else {
                self.tool_index.insert(tool.name.clone(), name.clone());
            }

            self.all_tools.push(registered);
        }

        // Index resources
        for res in &resources {
            let registered = RegisteredResource::new(res.clone(), &name);
            self.resource_index.insert(res.uri.clone(), name.clone());
            self.resource_index.insert(registered.fqn.clone(), name.clone());
            self.all_resources.push(registered);
        }

        // Index prompts
        for prompt in &prompts {
            let registered = RegisteredPrompt::new(prompt.clone(), &name);
            self.prompt_index.insert(prompt.name.clone(), name.clone());
            self.prompt_index.insert(registered.fqn.clone(), name.clone());
            self.all_prompts.push(registered);
        }

        self.servers.insert(
            name.clone(),
            UpstreamServer {
                name,
                config,
                tools,
                resources,
                prompts,
                connected,
            },
        );
    }

    /// Mark an upstream server as disconnected and purge its tools from routing
    pub fn mark_server_disconnected(&mut self, server_name: &str) -> usize {
        let server = match self.servers.get_mut(server_name) {
            Some(s) => s,
            None => return 0,
        };

        if !server.connected {
            return 0;
        }

        server.connected = false;
        let tool_count = server.tools.len();

        for tool in &server.tools {
            self.tool_index.remove(&tool.name);
            let fqn = format!("{}/{}", server_name, tool.name);
            self.tool_index.remove(&fqn);
        }

        for res in &server.resources {
            self.resource_index.remove(&res.uri);
            let fqn = format!("{}/{}", server_name, res.uri);
            self.resource_index.remove(&fqn);
        }

        for prompt in &server.prompts {
            self.prompt_index.remove(&prompt.name);
            let fqn = format!("{}/{}", server_name, prompt.name);
            self.prompt_index.remove(&fqn);
        }

        self.all_tools.retain(|t| t.server_name != server_name);
        self.all_resources.retain(|r| r.server_name != server_name);
        self.all_prompts.retain(|p| p.server_name != server_name);

        self.stdio_connections.remove(server_name);
        server.tools.clear();
        server.resources.clear();
        server.prompts.clear();

        warn!(
            "⚠️ Gateway: Server '{}' marked disconnected ({} tools unindexed)",
            server_name, tool_count
        );
        tool_count
    }

    /// Resolves tool execution target under ephemeral lock to avoid holding RwLock across network awaits
    pub fn resolve_tool_execution(
        &self,
        name_or_fqn: &str,
        arguments: Option<serde_json::Value>,
    ) -> anyhow::Result<ToolExecutionTarget> {
        let server_name = self
            .tool_index
            .get(name_or_fqn)
            .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found in any connected server", name_or_fqn))?;

        let server = self
            .servers
            .get(server_name)
            .ok_or_else(|| anyhow::anyhow!("Server '{}' not registered", server_name))?;

        if !server.connected {
            return Err(anyhow::anyhow!("Server '{}' is not connected", server_name));
        }

        let bare_name = if let Some((_, bare)) = name_or_fqn.split_once('/') {
            bare.to_string()
        } else {
            name_or_fqn.to_string()
        };

        if let Some(ref url) = server.config.url {
            let session_id = self.get_http_session_id(server_name);
            Ok(ToolExecutionTarget::Http {
                server_name: server_name.clone(),
                url: url.clone(),
                config: server.config.clone(),
                bare_name,
                arguments,
                client: self.http_client.clone(),
                session_id,
            })
        } else if let Some(conn) = self.stdio_connections.get(server_name) {
            Ok(ToolExecutionTarget::Stdio {
                conn: conn.clone(),
                bare_name,
                arguments,
            })
        } else {
            Err(anyhow::anyhow!("No active transport connection for '{}'", server_name))
        }
    }

    /// Dispatches a tool call to the owning upstream server
    pub async fn call_tool(
        &self,
        name_or_fqn: &str,
        arguments: Option<serde_json::Value>,
    ) -> anyhow::Result<serde_json::Value> {
        let target = self.resolve_tool_execution(name_or_fqn, arguments)?;
        target.execute().await
    }

    /// Read an upstream resource
    pub async fn read_resource(&self, uri: &str) -> anyhow::Result<serde_json::Value> {
        let server_name = self
            .resource_index
            .get(uri)
            .ok_or_else(|| anyhow::anyhow!("Resource '{}' not found in any connected server", uri))?;

        let server = self
            .servers
            .get(server_name)
            .ok_or_else(|| anyhow::anyhow!("Server '{}' not registered", server_name))?;

        if !server.connected {
            return Err(anyhow::anyhow!("Server '{}' is not connected", server_name));
        }

        if let Some(ref url) = server.config.url {
            let req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "resources/read",
                "params": { "uri": uri }
            });
            let resp = self.send_mcp_http_request(server_name, url, &server.config, &req).await?;
            resp.get("result").cloned().ok_or_else(|| anyhow::anyhow!("Resource read error: {:?}", resp.get("error")))
        } else if let Some(conn) = self.stdio_connections.get(server_name) {
            conn.send_request("resources/read", serde_json::json!({ "uri": uri })).await
        } else {
            Err(anyhow::anyhow!("No connection for server '{}'", server_name))
        }
    }

    /// Get all registered upstream tools
    pub fn get_all_tools(&self) -> Vec<RegisteredTool> {
        self.all_tools.clone()
    }

    /// Find server owning a tool
    pub fn find_tool_server(&self, name_or_fqn: &str) -> Option<String> {
        self.tool_index.get(name_or_fqn).cloned()
    }

    /// Return status summary for all managed servers
    pub fn get_server_statuses(&self) -> Vec<UpstreamServerStatus> {
        self.servers
            .values()
            .map(|s| UpstreamServerStatus {
                name: s.name.clone(),
                connected: s.connected,
                transport: if s.config.url.is_some() {
                    "http".to_string()
                } else {
                    "stdio".to_string()
                },
                tools_count: s.tools.len(),
                resources_count: s.resources.len(),
                prompts_count: s.prompts.len(),
            })
            .collect()
    }

    /// Converts all upstream tools into ChronoFact ToolCandidates for TF-IDF prompt pruning
    pub fn to_tool_candidates(&self) -> Vec<ToolCandidate> {
        self.all_tools
            .iter()
            .map(|tool| {
                let mut params = Vec::new();
                if let Some(ref schema) = tool.definition.input_schema {
                    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                        for key in props.keys() {
                            params.push(key.clone());
                        }
                    }
                }

                ToolCandidate::new(
                    &tool.definition.name,
                    tool.definition.description.as_deref().unwrap_or(""),
                    &tool.server_name,
                    params,
                )
            })
            .collect()
    }

    // ── Internal discovery implementations ──

    async fn discover_stdio(
        &mut self,
        config: &UpstreamServerConfig,
        req_timeout: Duration,
        handshake_timeout: Duration,
    ) -> (Vec<ToolDefinition>, Vec<ResourceDefinition>, Vec<PromptDefinition>, bool) {
        match StdioConnection::connect(config, self.death_tx.clone(), req_timeout, handshake_timeout).await {
            Ok((conn, caps)) => {
                let has_tools = caps.get("tools").is_some();
                let has_resources = caps.get("resources").is_some();
                let has_prompts = caps.get("prompts").is_some();

                let tools_res: Result<serde_json::Value, _> = conn.send_request("tools/list", serde_json::json!({})).await;
                let tools: Vec<ToolDefinition> = if has_tools {
                    tools_res
                        .ok()
                        .and_then(|r| r.get("tools").cloned())
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };

                let res_val: Result<serde_json::Value, _> = conn.send_request("resources/list", serde_json::json!({})).await;
                let resources: Vec<ResourceDefinition> = if has_resources {
                    res_val
                        .ok()
                        .and_then(|r| r.get("resources").cloned())
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };

                let prompts_val: Result<serde_json::Value, _> = conn.send_request("prompts/list", serde_json::json!({})).await;
                let prompts: Vec<PromptDefinition> = if has_prompts {
                    prompts_val
                        .ok()
                        .and_then(|r| r.get("prompts").cloned())
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };

                self.stdio_connections.insert(config.name.clone(), Arc::new(conn));
                (tools, resources, prompts, true)
            }
            Err(e) => {
                warn!("Gateway: Failed to connect stdio server '{}': {}", config.name, e);
                (Vec::new(), Vec::new(), Vec::new(), false)
            }
        }
    }

    /// Retrieve active MCP session ID for an HTTP upstream server
    pub fn get_http_session_id(&self, server_name: &str) -> Option<String> {
        self.http_sessions.read().ok().and_then(|s| s.get(server_name).cloned())
    }

    /// Parse an MCP HTTP response, supporting both standard JSON and Streamable HTTP Server-Sent Events (SSE)
    pub fn parse_mcp_response(content_type: Option<&str>, body_text: &str) -> anyhow::Result<serde_json::Value> {
        let trimmed = body_text.trim();
        if trimmed.is_empty() {
            return Ok(serde_json::json!({}));
        }

        let is_sse = content_type
            .map(|ct| ct.to_lowercase().contains("text/event-stream"))
            .unwrap_or(false)
            || (!trimmed.starts_with('{') && !trimmed.starts_with('[') && trimmed.contains("data:"));

        if is_sse {
            Self::parse_sse_stream(trimmed)
        } else {
            serde_json::from_str(trimmed)
                .map_err(|e| anyhow::anyhow!("Failed to parse JSON response: {} (Body: {})", e, trimmed))
        }
    }

    /// Extract the JSON-RPC response object from an SSE stream
    pub fn parse_sse_stream(body_text: &str) -> anyhow::Result<serde_json::Value> {
        let mut last_candidate = None;

        // Standard SSE event boundary: double newline (\n\n or \r\n\r\n)
        let normalized = body_text.replace("\r\n", "\n");
        let chunks: Vec<&str> = normalized.split("\n\n").collect();
        for chunk in chunks {
            let mut data_lines = Vec::new();
            for line in chunk.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("data:") {
                    let payload = rest.trim_start();
                    if !payload.is_empty() && payload != "[DONE]" {
                        data_lines.push(payload);
                    }
                }
            }

            if !data_lines.is_empty() {
                let merged = data_lines.join("\n");
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&merged) {
                    if parsed.get("result").is_some() || parsed.get("error").is_some() {
                        return Ok(parsed);
                    }
                    last_candidate = Some(parsed);
                }
            }
        }

        if let Some(cand) = last_candidate {
            return Ok(cand);
        }

        // Fallback: search each line starting with data:
        for line in normalized.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("data:") {
                let payload = rest.trim_start();
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(payload) {
                    if parsed.get("result").is_some() || parsed.get("error").is_some() {
                        return Ok(parsed);
                    }
                    last_candidate = Some(parsed);
                }
            }
        }

        last_candidate.ok_or_else(|| anyhow::anyhow!("No valid JSON-RPC message found in SSE stream: {}", body_text))
    }

    /// Send a Streamable HTTP-compliant MCP request with session tracking, Accept headers, and auth
    pub async fn send_mcp_http_request(
        &self,
        server_name: &str,
        url: &str,
        config: &UpstreamServerConfig,
        payload: &serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let mut builder = self.http_client.post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        // Attach Mcp-Session-Id if previously established for this upstream server
        if let Some(session_id) = self.get_http_session_id(server_name) {
            builder = builder.header("Mcp-Session-Id", session_id);
        }

        // Attach custom authentication headers from config
        if let Some(ref headers) = config.headers {
            for (k, v) in headers {
                builder = builder.header(k, v);
            }
        }

        let resp = builder.json(payload).send().await?;

        // Extract Mcp-Session-Id from response headers if present
        if let Some(sess_val) = resp.headers().get("mcp-session-id") {
            if let Ok(sess_str) = sess_val.to_str() {
                if let Ok(mut sessions) = self.http_sessions.write() {
                    sessions.insert(server_name.to_string(), sess_str.trim().to_string());
                }
            }
        }

        let status = resp.status();
        let content_type = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        let body_text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow::anyhow!("Upstream HTTP server returned status {}: {}", status, body_text));
        }

        Self::parse_mcp_response(content_type.as_deref(), &body_text)
    }

    async fn discover_http(
        &self,
        config: &UpstreamServerConfig,
    ) -> (Vec<ToolDefinition>, Vec<ResourceDefinition>, Vec<PromptDefinition>, bool) {
        let url = match config.url.as_ref() {
            Some(u) => u,
            None => return (Vec::new(), Vec::new(), Vec::new(), false),
        };

        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "chronofact-gateway",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        let init_resp = match self.send_mcp_http_request(&config.name, url, config, &init_req).await {
            Ok(j) => j,
            Err(e) => {
                warn!("Gateway: Failed to initialize HTTP server '{}': {}", config.name, e);
                return (Vec::new(), Vec::new(), Vec::new(), false);
            }
        };

        let caps = init_resp
            .get("result")
            .and_then(|r| r.get("capabilities"))
            .cloned()
            .unwrap_or_default();

        let notify_req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let _ = self.send_mcp_http_request(&config.name, url, config, &notify_req).await;

        let has_tools = caps.get("tools").is_some();
        let has_resources = caps.get("resources").is_some();
        let has_prompts = caps.get("prompts").is_some();

        let tools = if has_tools {
            let req = serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} });
            if let Ok(body) = self.send_mcp_http_request(&config.name, url, config, &req).await {
                body.get("result")
                    .and_then(|r| r.get("tools").cloned())
                    .and_then(|t| serde_json::from_value(t).ok())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let resources = if has_resources {
            let req = serde_json::json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/list", "params": {} });
            if let Ok(body) = self.send_mcp_http_request(&config.name, url, config, &req).await {
                body.get("result")
                    .and_then(|r| r.get("resources").cloned())
                    .and_then(|r| serde_json::from_value(r).ok())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let prompts = if has_prompts {
            let req = serde_json::json!({ "jsonrpc": "2.0", "id": 4, "method": "prompts/list", "params": {} });
            if let Ok(body) = self.send_mcp_http_request(&config.name, url, config, &req).await {
                body.get("result")
                    .and_then(|r| r.get("prompts").cloned())
                    .and_then(|p| serde_json::from_value(p).ok())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        (tools, resources, prompts, true)
    }
}
