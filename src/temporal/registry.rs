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
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
        };
        registry.seed_known_models();
        registry.load_environmental_overrides();
        registry
    }

    fn seed_known_models(&mut self) {
        let known = vec![
            // ==================== OpenAI GPT-6 Generation (September 2026) ====================
            ModelHorizon {
                model_id: "gpt-6-astra".to_string(),
                display_name: "GPT-6 Astra (Astra 6)".to_string(),
                vendor: "OpenAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2026, 9, 3).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                is_frontier: true,
                status: "Active (Frontier Flagship)".to_string(),
                notes: "OpenAI flagship frontier intelligence with 1.05M context window, autonomous computer use and advanced coding. Released September 3, 2026.".to_string(),
            },
            ModelHorizon {
                model_id: "gpt-6-1-sol".to_string(),
                display_name: "GPT-6.1 Sol (Sol 6.1)".to_string(),
                vendor: "OpenAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2026, 9, 29).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
                is_frontier: true,
                status: "Active (Frontier Flagship)".to_string(),
                notes: "OpenAI DevDay 2026 release with 1.05M context, near-Astra agentic coding performance at 1/5th the inference cost. Released September 29, 2026.".to_string(),
            },
            ModelHorizon {
                model_id: "gpt-5".to_string(),
                display_name: "GPT-5 (Orion)".to_string(),
                vendor: "OpenAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2025, 11, 1).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(),
                is_frontier: true,
                status: "Active".to_string(),
                notes: "OpenAI fifth-generation foundational intelligence.".to_string(),
            },
            ModelHorizon {
                model_id: "o3".to_string(),
                display_name: "OpenAI o3".to_string(),
                vendor: "OpenAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 11, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 12, 1).unwrap(),
                is_frontier: true,
                status: "Active (Reasoning)".to_string(),
                notes: "Full-scale reasoning model with extended test-time compute.".to_string(),
            },
            ModelHorizon {
                model_id: "o3-mini".to_string(),
                display_name: "OpenAI o3-mini".to_string(),
                vendor: "OpenAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 10, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 10, 31).unwrap(),
                is_frontier: true,
                status: "Active".to_string(),
                notes: "High-speed reasoning model for coding, math, and STEM.".to_string(),
            },
            ModelHorizon {
                model_id: "o1".to_string(),
                display_name: "OpenAI o1 (Legacy)".to_string(),
                vendor: "OpenAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2024, 12, 5).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2023, 10, 31).unwrap(),
                is_frontier: false,
                status: "Legacy Active".to_string(),
                notes: "First-generation OpenAI reasoning model.".to_string(),
            },
            ModelHorizon {
                model_id: "gpt-4o".to_string(),
                display_name: "GPT-4o (Legacy)".to_string(),
                vendor: "OpenAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2024, 5, 13).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2023, 10, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2023, 10, 31).unwrap(),
                is_frontier: false,
                status: "Legacy Active".to_string(),
                notes: "Flagship multimodal Omni model. Superseded by GPT-5 & GPT-6 series.".to_string(),
            },

            // ==================== Anthropic Claude 5.5 Generation (September 2026) ====================
            ModelHorizon {
                model_id: "claude-opus-5-5".to_string(),
                display_name: "Claude Opus 5.5".to_string(),
                vendor: "Anthropic".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2026, 9, 22).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                is_frontier: true,
                status: "Active (Frontier Flagship)".to_string(),
                notes: "Anthropic flagship frontier reasoning model. Released September 22, 2026.".to_string(),
            },
            ModelHorizon {
                model_id: "claude-sonnet-5-5".to_string(),
                display_name: "Claude Sonnet 5.5".to_string(),
                vendor: "Anthropic".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2026, 9, 28).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
                is_frontier: true,
                status: "Active (Frontier Flagship)".to_string(),
                notes: "Anthropic flagship coding intelligence. Released September 28, 2026.".to_string(),
            },
            ModelHorizon {
                model_id: "claude-haiku-5-5".to_string(),
                display_name: "Claude Haiku 5.5".to_string(),
                vendor: "Anthropic".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
                is_frontier: false,
                status: "Active".to_string(),
                notes: "Anthropic ultra-low latency generation 5.5 model.".to_string(),
            },
            ModelHorizon {
                model_id: "claude-3-7-sonnet".to_string(),
                display_name: "Claude 3.7 Sonnet".to_string(),
                vendor: "Anthropic".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2025, 2, 24).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 10, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 11, 1).unwrap(),
                is_frontier: false,
                status: "Deprecated (Superseded by 5.5)".to_string(),
                notes: "Superseded by Claude 4 and Claude 5.5 generations.".to_string(),
            },
            ModelHorizon {
                model_id: "claude-3-5-sonnet".to_string(),
                display_name: "Claude 3.5 Sonnet (Retired)".to_string(),
                vendor: "Anthropic".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2024, 6, 20).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 4, 30).unwrap(),
                is_frontier: false,
                status: "Retired (End of Life)".to_string(),
                notes: "Officially retired by Anthropic on October 28, 2025. API requests defunct.".to_string(),
            },
            ModelHorizon {
                model_id: "claude-3-opus".to_string(),
                display_name: "Claude 3 Opus (Retired)".to_string(),
                vendor: "Anthropic".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2024, 2, 29).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2023, 8, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2023, 8, 31).unwrap(),
                is_frontier: false,
                status: "Retired".to_string(),
                notes: "First-generation Claude 3 flagship. Retired.".to_string(),
            },

            // ==================== xAI Grok 4+ Generation (2025-2026) ====================
            ModelHorizon {
                model_id: "grok-4-7".to_string(),
                display_name: "Grok 4.7".to_string(),
                vendor: "xAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2026, 9, 21).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
                is_frontier: true,
                status: "Active (Frontier Flagship)".to_string(),
                notes: "Latest xAI frontier flagship trained on expanded Colossus supercluster. Released September 21, 2026.".to_string(),
            },
            ModelHorizon {
                model_id: "grok-4".to_string(),
                display_name: "Grok 4".to_string(),
                vendor: "xAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2025, 7, 9).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
                is_frontier: true,
                status: "Active".to_string(),
                notes: "Fourth-generation xAI frontier intelligence released July 9, 2025.".to_string(),
            },
            ModelHorizon {
                model_id: "grok-3".to_string(),
                display_name: "Grok 3 (Legacy Frontier)".to_string(),
                vendor: "xAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2025, 2, 17).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 11, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
                is_frontier: false,
                status: "Superseded by Grok 4+".to_string(),
                notes: "Launched February 2025 on initial Colossus cluster; superseded by Grok 4 & 4.7.".to_string(),
            },
            ModelHorizon {
                model_id: "grok-3-mini".to_string(),
                display_name: "Grok 3 Mini".to_string(),
                vendor: "xAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2025, 2, 17).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 11, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
                is_frontier: false,
                status: "Legacy Active".to_string(),
                notes: "High-speed reasoning model by xAI.".to_string(),
            },
            ModelHorizon {
                model_id: "grok-2".to_string(),
                display_name: "Grok 2 (Retired)".to_string(),
                vendor: "xAI".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2024, 8, 13).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
                is_frontier: false,
                status: "Retired".to_string(),
                notes: "Second-generation flagship from xAI. Deprecated.".to_string(),
            },

            // ==================== Google Gemini Models ====================
            ModelHorizon {
                model_id: "gemini-3-8-flash".to_string(),
                display_name: "Gemini 3.8 Flash".to_string(),
                vendor: "Google".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                is_frontier: true,
                status: "Active (Frontier Live)".to_string(),
                notes: "Google fast multimodal frontier model.".to_string(),
            },
            ModelHorizon {
                model_id: "gemini-3-8-pro".to_string(),
                display_name: "Gemini 3.8 Pro".to_string(),
                vendor: "Google".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
                is_frontier: true,
                status: "Active (Frontier Flagship)".to_string(),
                notes: "Google frontier multimodal reasoning flagship with deep code synthesis.".to_string(),
            },
            ModelHorizon {
                model_id: "gemini-2-0-pro".to_string(),
                display_name: "Gemini 2.0 Pro".to_string(),
                vendor: "Google".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2025, 2, 5).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 10, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 11, 1).unwrap(),
                is_frontier: true,
                status: "Active".to_string(),
                notes: "Google reasoning and coding model with 2M context.".to_string(),
            },

            // ==================== DeepSeek Models ====================
            ModelHorizon {
                model_id: "deepseek-r1".to_string(),
                display_name: "DeepSeek R1".to_string(),
                vendor: "DeepSeek".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2025, 1, 20).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 7, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 7, 31).unwrap(),
                is_frontier: true,
                status: "Active (Open Weights)".to_string(),
                notes: "Open-weights reasoning frontier model with pure RL.".to_string(),
            },
            ModelHorizon {
                model_id: "deepseek-v3".to_string(),
                display_name: "DeepSeek V3".to_string(),
                vendor: "DeepSeek".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2024, 12, 26).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 7, 1).unwrap(),
                is_frontier: true,
                status: "Active (Open Weights)".to_string(),
                notes: "671B MoE architecture with multi-head latent attention.".to_string(),
            },

            // ==================== Meta Models ====================
            ModelHorizon {
                model_id: "llama-3-3-70b".to_string(),
                display_name: "Llama 3.3 70B".to_string(),
                vendor: "Meta".to_string(),
                public_release_date: NaiveDate::from_ymd_opt(2024, 12, 6).unwrap(),
                estimated_training_freeze: NaiveDate::from_ymd_opt(2024, 7, 1).unwrap(),
                official_knowledge_cutoff: NaiveDate::from_ymd_opt(2024, 8, 1).unwrap(),
                is_frontier: true,
                status: "Active (Open Weights)".to_string(),
                notes: "Meta open-weights flagship with 128k context.".to_string(),
            },
        ];

        for m in known {
            // Insert primary key
            self.models.insert(m.model_id.to_lowercase(), m.clone());

            // Register aliases
            if m.model_id == "gpt-6-astra" {
                self.models.insert("astra-6".to_string(), m.clone());
                self.models.insert("astr-6".to_string(), m.clone());
                self.models.insert("astr6".to_string(), m.clone());
                self.models.insert("astra6".to_string(), m.clone());
                self.models.insert("gpt6-astra".to_string(), m.clone());
                self.models.insert("gpt-6".to_string(), m.clone());
                self.models.insert("gpt6".to_string(), m.clone());
            } else if m.model_id == "gpt-6-1-sol" {
                self.models.insert("sol-6-1".to_string(), m.clone());
                self.models.insert("sol-6.1".to_string(), m.clone());
                self.models.insert("sol6.1".to_string(), m.clone());
                self.models.insert("sol-6".to_string(), m.clone());
                self.models.insert("sol6".to_string(), m.clone());
                self.models.insert("gpt-6.1-sol".to_string(), m.clone());
            } else if m.model_id == "claude-opus-5-5" {
                self.models.insert("opus-5-5".to_string(), m.clone());
                self.models.insert("opus-5.5".to_string(), m.clone());
                self.models.insert("claude-5-5-opus".to_string(), m.clone());
                self.models.insert("opus5.5".to_string(), m.clone());
            } else if m.model_id == "claude-sonnet-5-5" {
                self.models.insert("sonnet-5-5".to_string(), m.clone());
                self.models.insert("sonnet-5.5".to_string(), m.clone());
                self.models.insert("claude-5-5-sonnet".to_string(), m.clone());
                self.models.insert("sonnet5.5".to_string(), m.clone());
            } else if m.model_id == "grok-4-7" {
                self.models.insert("grok-4.7".to_string(), m.clone());
                self.models.insert("grok4.7".to_string(), m.clone());
                self.models.insert("grok47".to_string(), m.clone());
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

        // 3. Fallback deterministic substring match (prioritize longest matching model key)
        let mut best_match: Option<&ModelHorizon> = None;
        let mut best_len = 0;
        for (k, v) in &candidates {
            let clean_k: String = k.chars().filter(|c| c.is_alphanumeric()).collect();
            if clean_key.contains(&clean_k) && clean_k.len() > best_len {
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

