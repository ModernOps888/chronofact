use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for an upstream MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamServerConfig {
    pub name: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub handshake_timeout_secs: Option<u64>,
    #[serde(default)]
    pub request_timeout_secs: Option<u64>,
}

fn default_enabled() -> bool {
    true
}

impl UpstreamServerConfig {
    pub fn stdio(name: impl Into<String>, command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            command: Some(command.into()),
            args,
            env: HashMap::new(),
            url: None,
            enabled: true,
            handshake_timeout_secs: Some(15),
            request_timeout_secs: Some(30),
        }
    }

    pub fn http(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            url: Some(url.into()),
            enabled: true,
            handshake_timeout_secs: Some(10),
            request_timeout_secs: Some(30),
        }
    }
}

/// MCP Tool Definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "inputSchema", default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

/// A registered tool tagged with its origin server and fully-qualified name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredTool {
    pub definition: ToolDefinition,
    pub server_name: String,
    pub fqn: String,
}

impl RegisteredTool {
    pub fn new(definition: ToolDefinition, server_name: impl Into<String>) -> Self {
        let server_name = server_name.into();
        let fqn = format!("{}/{}", server_name, definition.name);
        Self {
            definition,
            server_name,
            fqn,
        }
    }
}

/// MCP Resource Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    pub uri: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// A registered resource tagged with its origin server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredResource {
    pub definition: ResourceDefinition,
    pub server_name: String,
    pub fqn: String,
}

impl RegisteredResource {
    pub fn new(definition: ResourceDefinition, server_name: impl Into<String>) -> Self {
        let server_name = server_name.into();
        let fqn = format!("{}/{}", server_name, definition.uri);
        Self {
            definition,
            server_name,
            fqn,
        }
    }
}

/// MCP Prompt Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<serde_json::Value>>,
}

/// A registered prompt tagged with its origin server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredPrompt {
    pub definition: PromptDefinition,
    pub server_name: String,
    pub fqn: String,
}

impl RegisteredPrompt {
    pub fn new(definition: PromptDefinition, server_name: impl Into<String>) -> Self {
        let server_name = server_name.into();
        let fqn = format!("{}/{}", server_name, definition.name);
        Self {
            definition,
            server_name,
            fqn,
        }
    }
}

/// Parameters for calling a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// Status summary for an upstream server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamServerStatus {
    pub name: String,
    pub connected: bool,
    pub transport: String,
    pub tools_count: usize,
    pub resources_count: usize,
    pub prompts_count: usize,
}
