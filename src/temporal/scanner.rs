use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalScanResult {
    pub is_temporally_sensitive: bool,
    pub temporal_risk_score: f32, // 0.0 to 1.0
    pub detected_entities: Vec<String>,
    pub temporal_keywords: Vec<String>,
    pub mentioned_years: Vec<i32>,
    pub outdated_models_flagged: Vec<String>,
    pub requires_search: bool,
    pub search_query_suggestion: Option<String>,
}

pub struct TemporalScanner {
    year_regex: Regex,
    version_regex: Regex,
    keyword_regex: Regex,
    tech_entity_regex: Regex,
    outdated_model_regex: Regex,
    software_intent_regex: Regex,
}

impl Default for TemporalScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl TemporalScanner {
    pub fn new() -> Self {
        Self {
            year_regex: Regex::new(r"\b(20[2-3][0-9])\b").unwrap(),
            version_regex: Regex::new(r"\bv?(\d+\.\d+(\.\d+)?(-[a-zA-Z0-9.]+)?)\b").unwrap(),
            keyword_regex: Regex::new(r"(?i)\b(latest|newest|recent|recently|current|now|today|yesterday|tomorrow|this year|last month|changelog|roadmap|release[ds]?|deprecat(ed|ion)?|update[ds]?|breaking change[s]?)\b").unwrap(),
            tech_entity_regex: Regex::new(r"(?i)\b(claude|gpt-?[456o]?|gemini|astra(-?6)?|astr(-?6)?|sol(-?6(\.1)?)?|deepseek|llama|opus(-?5(\.5)?)?|sonnet(-?5(\.5)?)?|haiku|grok(-?[1234](\.[0-9])?)?|xai|openai|anthropic|deepmind|meta|o[134](-mini)?|react|next\.?js|vite|rust|cargo|tokio|axum|actix|serde|reqwest|sqlx|diesel|polars|tauri|bevy|tracing|clap|python|pip|uv|pydantic|fastapi|django|flask|torch|pytorch|tensorflow|numpy|pandas|scipy|scikit-learn|langchain|llamaindex|transformers|huggingface|celery|sqlalchemy|alembic|node\.?js|typescript|javascript|bun|deno|tailwind|kubernetes|docker|fastapi|astro|arxiv|biorxiv|swe-bench|mmlu|gpqa|sota|benchmark|rlhf|dpo|grpo)\b").unwrap(),
            outdated_model_regex: Regex::new(r"(?i)\b(claude(-|\s)?3([.-][57])?(-|\s)?(sonnet|haiku|opus)|sonnet(-|\s)?3([.-][57])|opus(-|\s)?3|grok(-|\s)?[123](\.0|\.5)?|gpt(-|\s)?4(o|-turbo|-mini)?)\b").unwrap(),
            software_intent_regex: Regex::new(r"(?i)\b(how to|how do i|implement|best practice|pattern|architecture|setup|config|migration|migrate|breaking change|upgrade|install|dependency|package|crate|library|framework|module|api|sdk|cli|vendor|owner|who made|who developed)\b").unwrap(),
        }
    }

    pub fn scan(&self, query: &str) -> TemporalScanResult {
        let mut temporal_keywords = Vec::new();
        for mat in self.keyword_regex.find_iter(query) {
            let kw = mat.as_str().to_lowercase();
            if !temporal_keywords.contains(&kw) {
                temporal_keywords.push(kw);
            }
        }

        let mut mentioned_years = Vec::new();
        for cap in self.year_regex.captures_iter(query) {
            if let Some(m) = cap.get(1) {
                if let Ok(yr) = m.as_str().parse::<i32>() {
                    if !mentioned_years.contains(&yr) {
                        mentioned_years.push(yr);
                    }
                }
            }
        }

        let mut detected_entities = Vec::new();
        for mat in self.tech_entity_regex.find_iter(query) {
            let ent = mat.as_str().to_lowercase();
            if !detected_entities.contains(&ent) {
                detected_entities.push(ent);
            }
        }

        for cap in self.version_regex.captures_iter(query) {
            if let Some(m) = cap.get(0) {
                let v = m.as_str().to_string();
                if !detected_entities.contains(&v) {
                    detected_entities.push(v);
                }
            }
        }

        let mut outdated_models_flagged = Vec::new();
        for mat in self.outdated_model_regex.find_iter(query) {
            let matched_str = mat.as_str().to_lowercase();
            let note = if matched_str.contains("3.5") || matched_str.contains("3-5") {
                format!("'{}' is officially RETIRED (EOL Oct 2025). Active successor: Claude Sonnet 5.5 / Opus 5.5", mat.as_str())
            } else if matched_str.contains("3.7") || matched_str.contains("3-7") {
                format!("'{}' is DEPRECATED and superseded by Claude 5.5 generation.", mat.as_str())
            } else if matched_str.contains("grok") && (matched_str.contains('3') || matched_str.contains('2') || matched_str.contains('1')) {
                format!("'{}' is SUPERSEDED by Grok 4 and Grok 4.7 (Sep 2026).", mat.as_str())
            } else if matched_str.contains("gpt-4") || matched_str.contains("gpt 4") || matched_str.contains("gpt4") {
                format!("'{}' is legacy generation. Current frontier is GPT-5 and o3.", mat.as_str())
            } else {
                format!("'{}' is a legacy/superseded model version.", mat.as_str())
            };
            if !outdated_models_flagged.contains(&note) {
                outdated_models_flagged.push(note);
            }
        }

        // Scan for software intent (separate from pure temporal keywords)
        let mut has_software_intent = false;
        for mat in self.software_intent_regex.find_iter(query) {
            has_software_intent = true;
            let kw = mat.as_str().to_lowercase();
            if (kw.contains("breaking") || kw.contains("migrat") || kw.contains("upgrade"))
                && !temporal_keywords.contains(&kw)
            {
                temporal_keywords.push(kw);
            }
        }

        // Factual misattribution check for frontier models across major labs
        let query_lower = query.to_lowercase();
        if (query_lower.contains("astra") || query_lower.contains("sol 6") || query_lower.contains("astr 6") || query_lower.contains("sol-6"))
            && (query_lower.contains("google") || query_lower.contains("deepmind") || query_lower.contains("anthropic") || query_lower.contains("meta") || query_lower.contains("xai"))
        {
            let note = "FACTUAL CONTRADICTION WARNING: Astra 6 (GPT-6 Astra) and Sol 6.1 (GPT-6.1 Sol) are OpenAI models (launched Sep 2026), NOT Google DeepMind, Anthropic, Meta, or xAI.".to_string();
            if !outdated_models_flagged.contains(&note) {
                outdated_models_flagged.push(note);
            }
        }
        if (query_lower.contains("claude") || query_lower.contains("opus 5") || query_lower.contains("sonnet 5"))
            && (query_lower.contains("google") || query_lower.contains("openai") || query_lower.contains("meta"))
        {
            let note = "FACTUAL CONTRADICTION WARNING: Claude models (Opus 5.5, Sonnet 5.5) are developed by Anthropic, NOT Google, OpenAI, or Meta.".to_string();
            if !outdated_models_flagged.contains(&note) {
                outdated_models_flagged.push(note);
            }
        }

        // Calculate Temporal Risk Score (0.0 to 1.0)
        let mut score: f32 = 0.0;

        // Keywords boost score
        if !temporal_keywords.is_empty() {
            score += (temporal_keywords.len() as f32 * 0.25).min(0.5);
        }

        // Outdated models or factual misattributions automatically trigger high temporal scrutiny
        if !outdated_models_flagged.is_empty() {
            score += 0.50;
        }

        // Recent years boost score (e.g. 2024, 2025, 2026)
        for &yr in &mentioned_years {
            if yr >= 2024 {
                score += 0.35;
            }
        }

        // Tech entities + versions combined indicate volatile software specs
        if !detected_entities.is_empty() {
            score += (detected_entities.len() as f32 * 0.15).min(0.4);
        }

        // Software implementation intent on tech entities indicates volatile library APIs
        if has_software_intent && !detected_entities.is_empty() {
            score += 0.25;
        }

        let temporal_risk_score = score.min(1.0);
        let is_temporally_sensitive = temporal_risk_score >= 0.35;
        let requires_search = temporal_risk_score >= 0.35 || !outdated_models_flagged.is_empty();

        let search_query_suggestion = if requires_search {
            let primary_entities = detected_entities.join(" ");
            let clean_query = query.replace('?', "");
            Some(format!("{} latest release changelog 2026", if primary_entities.is_empty() { clean_query } else { primary_entities }))
        } else {
            None
        };

        TemporalScanResult {
            is_temporally_sensitive,
            temporal_risk_score,
            detected_entities,
            temporal_keywords,
            mentioned_years,
            outdated_models_flagged,
            requires_search,
            search_query_suggestion,
        }
    }
}
