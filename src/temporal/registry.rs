//! Configurable Model Horizon Registry & Calibration Policy Engine
//!
//! # Epistemic Policy Disclosure
//! The model records seeded by default represent configurable policy templates,
//! simulation baselines, and historical calibration horizons. They are explicitly
//! NOT dogmatic or unverifiable absolute ground truth.
//!
//! In production agent deployments, model cutoffs and freeze dates are dynamic.
//! Teams should supply their own organizationally vetted policy configurations
//! (via API or external config). This built-in registry serves as an extensible
//! calibration reference for calculating knowledge lag deltas and injecting
//! `<chronofact_temporal_anchor>` system guidance.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Built-in model horizon policy data, embedded at compile time.
pub const EMBEDDED_MODEL_HORIZON_JSON: &str = include_str!("../../data/model_horizon.json");

/// Environment variable naming a JSON file (same schema as `data/model_horizon.json`) that
/// replaces the embedded seed data at runtime.
pub const MODEL_HORIZON_ENV_VAR: &str = "CHRONOFACT_MODEL_HORIZON";

/// One model record in the horizon data file: the `ModelHorizon` fields plus lookup aliases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHorizonEntry {
    #[serde(flatten)]
    pub horizon: ModelHorizon,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// Top-level schema of the model horizon data file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHorizonData {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub description: String,
    pub models: Vec<ModelHorizonEntry>,
}

/// Where the seed data of a registry came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HorizonDataSource {
    Embedded,
    Override(String),
}

impl ModelHorizonData {
    pub fn parse(json_str: &str) -> Result<Self, String> {
        let data: ModelHorizonData = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse model horizon data: {}", e))?;
        if data.models.is_empty() {
            return Err("Model horizon data contains no models".to_string());
        }
        Ok(data)
    }

    pub fn embedded() -> Self {
        Self::parse(EMBEDDED_MODEL_HORIZON_JSON)
            .expect("embedded data/model_horizon.json must be valid (checked by tests)")
    }

    /// Loads the override file at `path` if given and valid; otherwise logs a warning (when a path
    /// was given) and returns the embedded data. Never panics on a bad override.
    pub fn load_with_override(path: Option<&str>) -> (Self, HorizonDataSource) {
        let Some(path) = path.map(str::trim).filter(|p| !p.is_empty()) else {
            return (Self::embedded(), HorizonDataSource::Embedded);
        };
        match std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read '{}': {}", path, e))
            .and_then(|content| Self::parse(&content))
        {
            Ok(data) => (data, HorizonDataSource::Override(path.to_string())),
            Err(e) => {
                tracing::warn!(
                    "{}={} is invalid ({}); falling back to embedded model horizon data",
                    MODEL_HORIZON_ENV_VAR,
                    path,
                    e
                );
                (Self::embedded(), HorizonDataSource::Embedded)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHorizon {
    pub model_id: String,
    pub display_name: String,
    pub vendor: String,
    pub public_release_date: NaiveDate,
    pub estimated_training_freeze: NaiveDate,
    pub official_knowledge_cutoff: NaiveDate,
    pub is_frontier: bool,
    pub status: String,
    pub notes: String,
}

pub struct ModelRegistry {
    models: HashMap<String, ModelHorizon>,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelRegistry {
    /// Builds the registry from the embedded `data/model_horizon.json`, or from the file named by
    /// `CHRONOFACT_MODEL_HORIZON` when set and valid, then applies the merge-style overrides.
    pub fn new() -> Self {
        let override_path = std::env::var(MODEL_HORIZON_ENV_VAR).ok();
        let (data, _source) = ModelHorizonData::load_with_override(override_path.as_deref());
        Self::from_data(data)
    }

    /// Builds a registry from explicit horizon data, then applies the merge-style overrides.
    pub fn from_data(data: ModelHorizonData) -> Self {
        let mut registry = Self {
            models: HashMap::new(),
        };
        registry.seed_from_data(data);
        registry.load_environmental_overrides();
        registry
    }

    fn seed_from_data(&mut self, data: ModelHorizonData) {
        for entry in data.models {
            let m = entry.horizon;
            // Insert primary key
            self.models.insert(m.model_id.to_lowercase(), m.clone());
            // Register normalized aliases
            for alias in entry.aliases {
                let norm_alias = alias.to_lowercase().trim().replace(' ', "-");
                self.models.insert(norm_alias, m.clone());
            }
        }
    }

    pub fn lookup(&self, model_id: &str) -> ModelHorizon {
        let key = model_id.to_lowercase().trim().replace(' ', "-");
        
        // 1. Direct exact match
        if let Some(m) = self.models.get(&key) {
            return m.clone();
        }

        // 2. Normalized alphanumeric match
        let clean_key: String = key.chars().filter(|c| c.is_alphanumeric()).collect();
        let mut candidates: Vec<(&String, &ModelHorizon)> = self.models.iter().collect();
        candidates.sort_by_key(|(k, _)| *k);

        for (k, v) in &candidates {
            let clean_k: String = k.chars().filter(|c| c.is_alphanumeric()).collect();
            if clean_key == clean_k {
                return (*v).clone();
            }
        }

        // 3. Fallback deterministic substring match (prioritize longest matching model key;
        //    require clean_k.len() >= 4 so 2-char keys like "o1"/"o3" never false-match "nemo12b" or "pro1")
        let mut best_match: Option<&ModelHorizon> = None;
        let mut best_len = 0;
        for (k, v) in &candidates {
            let clean_k: String = k.chars().filter(|c| c.is_alphanumeric()).collect();
            if clean_k.len() >= 4 && clean_key.contains(&clean_k) && clean_k.len() > best_len {
                best_len = clean_k.len();
                best_match = Some(v);
            }
        }
        if let Some(m) = best_match {
            return (*m).clone();
        }

        // 4. Reverse contains (only if clean_key has sufficient specificity >= 5 chars)
        if clean_key.len() >= 5 {
            for (k, v) in &candidates {
                let clean_k: String = k.chars().filter(|c| c.is_alphanumeric()).collect();
                if clean_k.contains(&clean_key) {
                    return (*v).clone();
                }
            }
        }

        // Generic fallback model with conservative 6-month historical freeze
        let now = chrono::Utc::now().naive_utc().date();
        let estimated_cutoff = now
            .checked_sub_signed(chrono::Duration::days(180))
            .unwrap_or(now);
        let estimated_freeze = now
            .checked_sub_signed(chrono::Duration::days(270))
            .unwrap_or(now);

        ModelHorizon {
            model_id: model_id.to_string(),
            display_name: format!("Generic Model ({})", model_id),
            vendor: "Unknown/Generic".to_string(),
            public_release_date: now,
            estimated_training_freeze: estimated_freeze,
            official_knowledge_cutoff: estimated_cutoff,
            is_frontier: false,
            status: "Unregistered (Fallback)".to_string(),
            notes: "Dynamically estimated 6-9 month knowledge lag baseline".to_string(),
        }
    }

    pub fn list_models(&self) -> Vec<ModelHorizon> {
        let mut seen = std::collections::HashSet::new();
        let mut list: Vec<ModelHorizon> = Vec::new();
        
        for m in self.models.values() {
            if seen.insert(m.model_id.clone()) {
                list.push(m.clone());
            }
        }
        list.sort_by(|a, b| b.public_release_date.cmp(&a.public_release_date));
        list
    }

    /// Dynamically registers or updates a model horizon policy.
    /// This allows users and teams to configure their own custom models and cutoffs.
    pub fn register_model(&mut self, horizon: ModelHorizon) {
        let key = horizon.model_id.to_lowercase();
        self.models.insert(key, horizon);
    }

    /// Checks for environment variable overrides or local configuration files to prevent cutoff drift.
    pub fn load_environmental_overrides(&mut self) {
        // 1. Check CHRONOFACT_MODELS_OVERRIDE / CHRONOFACT_MODELS_CONFIG / CHRONOFACT_MODELS_JSON
        for env_key in &["CHRONOFACT_MODELS_OVERRIDE", "CHRONOFACT_MODELS_CONFIG", "CHRONOFACT_MODELS_JSON"] {
            if let Ok(env_val) = std::env::var(env_key) {
                let trimmed = env_val.trim();
                if trimmed.starts_with('[') || trimmed.starts_with('{') {
                    let _ = self.load_from_json(trimmed);
                } else if std::path::Path::new(trimmed).exists() {
                    let _ = self.load_from_file(trimmed);
                }
            }
        }

        // 2. Check local fallback configuration files
        for fallback_path in &["chronofact_models.json", "config/models_override.json", "../config/models_override.json"] {
            if std::path::Path::new(fallback_path).exists() {
                let _ = self.load_from_file(fallback_path);
                break;
            }
        }
    }

    /// Dynamically loads model horizons from a JSON array string.
    pub fn load_from_json(&mut self, json_str: &str) -> Result<usize, String> {
        let parsed: Vec<ModelHorizon> = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse models JSON: {}", e))?;
        let count = parsed.len();
        for m in parsed {
            self.register_model(m);
        }
        Ok(count)
    }

    /// Dynamically loads model horizons from a local JSON file.
    pub fn load_from_file(&mut self, path: &str) -> Result<usize, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read models file at '{}': {}", path, e))?;
        self.load_from_json(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_4_argon_registration_and_aliases() {
        let reg = ModelRegistry::new();
        for alias in &[
            "gemini-4-argon",
            "google/gemini-4-argon",
            "gemini-4.0-argon",
            "gemini-4",
            "gemini4-argon",
            "argon",
        ] {
            let h = reg.lookup(alias);
            assert_eq!(h.model_id, "gemini-4-argon", "failed alias: {}", alias);
            assert_eq!(h.vendor, "Google DeepMind");
            assert!(h.is_frontier);
        }
    }

    #[test]
    fn test_short_key_collision_prevention() {
        let reg = ModelRegistry::new();
        // Previously, any unregistered model containing 'o' and '1' (like "cohere-v1-command")
        // matched "o1" via 2-char substring check in Step 3.
        let fallback = reg.lookup("cohere-v1-command");
        assert_ne!(fallback.model_id, "o1", "2-char 'o1' must not hijack unrelated model strings");
        assert_eq!(fallback.vendor, "Unknown/Generic");
    }
}
