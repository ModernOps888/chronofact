use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub db_path: String,
    pub log_level: String,
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

        let db_path = env::var("CHRONOFACT_DB").unwrap_or_else(|_| "chronofact_memory.db".to_string());

        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        Self {
            port,
            db_path,
            log_level,
        }
    }
}
