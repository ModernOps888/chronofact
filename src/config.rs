use std::env;
use std::fs;
use std::path::Path;
use crate::gateway::UpstreamServerConfig;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub db_path: String,
    pub log_level: String,
    pub servers: Vec<UpstreamServerConfig>,
    pub servers_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self::load()
    }
}

impl Config {
    pub fn load() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(3030);

        let db_path = env::var("CHRONOFACT_DB").unwrap_or_else(|_| {
            if Path::new("C:\\chronofact").exists() {
                "C:\\chronofact\\chronofact_memory.db".to_string()
            } else {
                "chronofact_memory.db".to_string()
            }
        });

        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        let servers_path = env::var("CHRONOFACT_SERVERS").ok().or_else(|| {
            if Path::new("C:\\chronofact\\servers.json").exists() {
                Some("C:\\chronofact\\servers.json".to_string())
            } else if Path::new("servers.json").exists() {
                Some("servers.json".to_string())
            } else {
                None
            }
        });

        let servers = if let Some(ref p) = servers_path {
            Self::load_servers_from_file(p).unwrap_or_default()
        } else {
            Vec::new()
        };

        Self {
            port,
            db_path,
            log_level,
            servers,
            servers_path,
        }
    }

    pub fn load_servers_from_file(path: &str) -> anyhow::Result<Vec<UpstreamServerConfig>> {
        let content = fs::read_to_string(path)?;
        let parsed: serde_json::Value = serde_json::from_str(&content)?;
        
        // Supports either {"servers": [...]} or direct array [...] or {"mcpServers": {"server_name": {...}}} (Claude/Cursor format)
        if let Some(arr) = parsed.get("servers").and_then(|v| v.as_array()) {
            let list = serde_json::from_value(serde_json::Value::Array(arr.clone()))?;
            Ok(list)
        } else if let Some(arr) = parsed.as_array() {
            let list = serde_json::from_value(serde_json::Value::Array(arr.clone()))?;
            Ok(list)
        } else if let Some(mcp) = parsed.get("mcpServers").and_then(|v| v.as_object()) {
            let mut list = Vec::new();
            for (name, obj) in mcp {
                let cmd = obj.get("command").and_then(|c| c.as_str()).map(|s| s.to_string());
                let args = obj
                    .get("args")
                    .and_then(|a| a.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let url = obj.get("url").and_then(|u| u.as_str()).map(|s| s.to_string());
                let env = obj
                    .get("env")
                    .and_then(|e| e.as_object())
                    .map(|map| {
                        map.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();

                list.push(UpstreamServerConfig {
                    name: name.clone(),
                    command: cmd,
                    args,
                    env,
                    url,
                    enabled: true,
                    handshake_timeout_secs: Some(15),
                    request_timeout_secs: Some(30),
                });
            }
            Ok(list)
        } else {
            Ok(Vec::new())
        }
    }
}

