use std::env;
use std::fs;
use std::path::PathBuf;
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

        let db_path = Self::discover_db_path();
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let servers_path = Self::discover_servers_path();

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

    /// Cross-platform discovery for servers.json configuration
    pub fn discover_servers_path() -> Option<String> {
        if let Ok(path) = env::var("CHRONOFACT_SERVERS") {
            if !path.trim().is_empty() {
                return Some(path);
            }
        }

        let mut candidates = Vec::new();

        // 1. Current working directory
        candidates.push(PathBuf::from("servers.json"));

        // 2. XDG_CONFIG_HOME / ~/.config on Unix/Linux/macOS
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            candidates.push(PathBuf::from(xdg).join("chronofact").join("servers.json"));
        }
        if let Ok(home) = env::var("HOME") {
            candidates.push(PathBuf::from(&home).join(".config").join("chronofact").join("servers.json"));
            candidates.push(PathBuf::from(&home).join(".chronofact").join("servers.json"));
        }

        // 3. Windows roaming AppData / UserProfile
        if let Ok(appdata) = env::var("APPDATA") {
            candidates.push(PathBuf::from(appdata).join("chronofact").join("servers.json"));
        }
        if let Ok(userprofile) = env::var("USERPROFILE") {
            candidates.push(PathBuf::from(userprofile).join(".chronofact").join("servers.json"));
        }

        // 4. Legacy Windows fallback
        candidates.push(PathBuf::from("C:\\chronofact\\servers.json"));

        for candidate in candidates {
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }

        None
    }

    /// Cross-platform discovery for SQLite database path
    pub fn discover_db_path() -> String {
        if let Ok(path) = env::var("CHRONOFACT_DB") {
            if !path.trim().is_empty() {
                return path;
            }
        }

        let mut candidates = Vec::new();

        // 1. Current working directory if exists
        candidates.push(PathBuf::from("chronofact_memory.db"));

        // 2. XDG_DATA_HOME / ~/.local/share on Unix/Linux/macOS
        if let Ok(xdg) = env::var("XDG_DATA_HOME") {
            candidates.push(PathBuf::from(xdg).join("chronofact").join("chronofact_memory.db"));
        }
        if let Ok(home) = env::var("HOME") {
            candidates.push(PathBuf::from(&home).join(".local").join("share").join("chronofact").join("chronofact_memory.db"));
            candidates.push(PathBuf::from(&home).join(".chronofact").join("chronofact_memory.db"));
        }

        // 3. Windows AppData / UserProfile
        if let Ok(appdata) = env::var("APPDATA") {
            candidates.push(PathBuf::from(appdata).join("chronofact").join("chronofact_memory.db"));
        }
        if let Ok(userprofile) = env::var("USERPROFILE") {
            candidates.push(PathBuf::from(userprofile).join(".chronofact").join("chronofact_memory.db"));
        }

        // 4. Legacy Windows fallback
        candidates.push(PathBuf::from("C:\\chronofact\\chronofact_memory.db"));

        for candidate in candidates {
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }

        "chronofact_memory.db".to_string()
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
                let headers = obj
                    .get("headers")
                    .and_then(|h| h.as_object())
                    .map(|map| {
                        map.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    });

                list.push(UpstreamServerConfig {
                    name: name.clone(),
                    command: cmd,
                    args,
                    env,
                    url,
                    headers,
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
