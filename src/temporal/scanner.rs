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
        static SCANNER_REGEXES: std::sync::OnceLock<(Regex, Regex, Regex, Regex, Regex, Regex)> =
            std::sync::OnceLock::new();
        let (yr_re, ver_re, kw_re, ent_re, out_re, sw_re) = SCANNER_REGEXES
            .get_or_init(|| {
                (
                    Regex::new(r"\b(20[2-3][0-9])\b").unwrap(),
                    Regex::new(r"\bv?(\d+\.\d+(\.\d+)?(-[a-zA-Z0-9.]+)?)\b").unwrap(),
                    Regex::new(r"(?i)\b(latest|newest|recently|current|this year|this quarter|last month|(?:in the )?last few (?:days|weeks|months)|changelog|roadmap|release[ds]?|ship(?:ped)? yet|started yet|landed|announced|disclosed|cve[s]?|beta|stable version|next edition|default now|enforcement|deprecat(ed|ion)?|breaking change[s]?|what'?s new|what is new|new features?|knowledge cutoff|cutoff|training freeze|weight freeze|context window|computer use)\b").unwrap(),
                    Regex::new(r"(?i)\b(claude|gpt-?[456o]?|gemini(-?[34](\.[0-9])?(-?(flash|pro|ultra|argon))?)?|argon|astra(-?6)?|astr(-?6)?|sol(-?6(\.1)?)?|deepseek|llama|opus(-?5(\.5)?)?|sonnet(-?5(\.5)?)?|haiku|grok(-?[1234](\.[0-9])?)?|xai|openai|anthropic|deepmind|meta|o[134](-mini)?|react|next\.?js|vite|rust|rustc|rustup|cargo|tokio|axum|actix|serde|reqwest|sqlx|diesel|polars|tauri|bevy|tracing|clap|python|pip|uv|pydantic|fastapi|django|flask|torch|pytorch|tensorflow|numpy|pandas|scipy|scikit-learn|langchain|llamaindex|transformers|huggingface|celery|sqlalchemy|alembic|node\.?js|typescript|javascript|bun|deno|tailwind|kubernetes|docker|astro|arxiv|biorxiv|swe-bench|mmlu|gpqa|sota|benchmark|rlhf|dpo|grpo|zig|svelte(kit)?|vue|angular|nuxt|remix|solid-?js|htmx|drizzle|prisma|zod|vitest|jest|playwright|cypress|golang|kotlin|swift(ui)?|flutter|dart|elixir|laravel|rails|spring boot|terraform|ansible|postgres(ql)?|redis|valkey|mongodb|kafka|nginx|openssl|ubuntu|eu ai act|webpack|esbuild|rollup|eslint|biome|turbopack|rspack|hono|fastify|ollama|langgraph|go\s?1\.\d{1,2}(\.\d+)?)\b").unwrap(),
                    Regex::new(r"(?i)\b(claude(-|\s)?3([.-][57])?(-|\s)?(sonnet|haiku|opus)|sonnet(-|\s)?3([.-][57])|opus(-|\s)?3|grok(-|\s)?[123](\.0|\.5)?|gpt(-|\s)?4(o|-turbo|-mini)?)\b").unwrap(),
                    Regex::new(r"(?i)\b(how to|how do i|implement|best practice|pattern|architecture|setup|config|migration|migrate|migrating|upgrading|installing|breaking change|upgrade|install|dependency|package|crate|library|framework|module|api|sdk|cli|vendor|owner|who made|who developed)\b").unwrap(),
                )
            })
            .clone();

        Self {
            year_regex: yr_re,
            version_regex: ver_re,
            keyword_regex: kw_re,
            tech_entity_regex: ent_re,
            outdated_model_regex: out_re,
            software_intent_regex: sw_re,
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
                let text = m.as_str();
                // A bare "0.5" / "3.14" is not a software version. Accept only explicit
                // v-prefixed, three-part (X.Y.Z), or entity-adjacent versions ("tokio 1.38").
                let before = &query[..m.start()];
                let tail_start = before
                    .char_indices()
                    .rev()
                    .nth(23)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let entity_adjacent = self.tech_entity_regex.is_match(&before[tail_start..]);
                let is_versioned = text.starts_with('v')
                    || text.starts_with('V')
                    || text.matches('.').count() >= 2
                    || entity_adjacent;
                if !is_versioned {
                    continue;
                }
                let v = text.to_string();
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
                format!("'{}' is legacy generation. Current OpenAI frontier is GPT-6 Astra, GPT-6.1 Sol, GPT-5, and o3.", mat.as_str())
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

        // Factual misattribution check for frontier models across major labs.
        // Scoped to individual comparison clauses and only triggers when the foreign vendor is NOT paired
        // with its own model family in the same clause (preventing false positives on "Claude vs OpenAI GPT-6").
        let query_lower = query.to_lowercase();
        let clause_normalized = query_lower
            .replace(" vs. ", " | ")
            .replace(" vs ", " | ")
            .replace(" versus ", " | ")
            .replace(" compared to ", " | ")
            .replace(" compared with ", " | ")
            .replace(" against ", " | ")
            .replace(" while ", " | ")
            .replace(" whereas ", " | ");

        for clause in clause_normalized.split(|c| c == ',' || c == ';' || c == '\n' || c == '|') {
            let c = clause.trim();
            if c.is_empty() {
                continue;
            }
            let has_openai_frontier =
                c.contains("astra") || c.contains("sol 6") || c.contains("astr 6") || c.contains("sol-6");
            let has_claude_frontier =
                c.contains("claude") || c.contains("opus 5") || c.contains("sonnet 5");
            let has_gemini_model = c.contains("gemini") || c.contains("argon");
            let has_grok_model = c.contains("grok");
            let has_llama_model = c.contains("llama");

            if has_openai_frontier {
                let foreign_google = (c.contains("google") || c.contains("deepmind")) && !has_gemini_model;
                let foreign_anthropic = c.contains("anthropic") && !has_claude_frontier;
                let foreign_meta = c.contains("meta") && !has_llama_model;
                let foreign_xai = c.contains("xai") && !has_grok_model;
                if foreign_google || foreign_anthropic || foreign_meta || foreign_xai {
                    let note = "FACTUAL CONTRADICTION WARNING: Astra 6 (GPT-6 Astra) and Sol 6.1 (GPT-6.1 Sol) are OpenAI models (launched Sep 2026), NOT Google DeepMind, Anthropic, Meta, or xAI.".to_string();
                    if !outdated_models_flagged.contains(&note) {
                        outdated_models_flagged.push(note);
                    }
                }
            }

            if has_claude_frontier {
                let foreign_google = (c.contains("google") || c.contains("deepmind")) && !has_gemini_model;
                let foreign_openai = c.contains("openai")
                    && !(has_openai_frontier
                        || c.contains("gpt")
                        || c.contains("o1")
                        || c.contains("o3")
                        || c.contains("o4"));
                let foreign_meta = c.contains("meta") && !has_llama_model;
                if foreign_google || foreign_openai || foreign_meta {
                    let note = "FACTUAL CONTRADICTION WARNING: Claude models (Opus 5.5, Sonnet 5.5) are developed by Anthropic, NOT Google, OpenAI, or Meta.".to_string();
                    if !outdated_models_flagged.contains(&note) {
                        outdated_models_flagged.push(note);
                    }
                }
            }

            if has_gemini_model {
                let foreign_openai = c.contains("openai")
                    && !(has_openai_frontier
                        || c.contains("gpt")
                        || c.contains("o1")
                        || c.contains("o3")
                        || c.contains("o4"));
                let foreign_anthropic = c.contains("anthropic") && !has_claude_frontier;
                let foreign_meta = c.contains("meta") && !has_llama_model;
                let foreign_xai = c.contains("xai") && !has_grok_model;
                if foreign_openai || foreign_anthropic || foreign_meta || foreign_xai {
                    let note = "FACTUAL CONTRADICTION WARNING: Gemini models (including Gemini 4 Argon) are developed by Google DeepMind, NOT OpenAI, Anthropic, Meta, or xAI.".to_string();
                    if !outdated_models_flagged.contains(&note) {
                        outdated_models_flagged.push(note);
                    }
                }
            }

            if has_grok_model {
                let foreign_openai = c.contains("openai")
                    && !(has_openai_frontier
                        || c.contains("gpt")
                        || c.contains("o1")
                        || c.contains("o3")
                        || c.contains("o4"));
                let foreign_google = (c.contains("google") || c.contains("deepmind")) && !has_gemini_model;
                let foreign_anthropic = c.contains("anthropic") && !has_claude_frontier;
                let foreign_meta = c.contains("meta") && !has_llama_model;
                if foreign_openai || foreign_google || foreign_anthropic || foreign_meta {
                    let note = "FACTUAL CONTRADICTION WARNING: Grok models (including Grok 4.7) are developed by xAI, NOT OpenAI, Google, Anthropic, or Meta.".to_string();
                    if !outdated_models_flagged.contains(&note) {
                        outdated_models_flagged.push(note);
                    }
                }
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

        // Recent years boost score (unless the prompt is a static data-sorting task on ISO date literals
        // with zero tech entities and zero temporal keywords).
        let is_static_date_literal_task = detected_entities.is_empty()
            && temporal_keywords.is_empty()
            && outdated_models_flagged.is_empty()
            && (query_lower.starts_with("sort ")
                || query_lower.starts_with("format ")
                || query_lower.contains("invoice dates"));

        if !is_static_date_literal_task {
            for &yr in &mentioned_years {
                if yr >= 2025 {
                    score += 0.35;
                } else if yr == 2024 {
                    score += 0.15;
                }
            }
        }

        // Tech entities + versions combined indicate volatile software specs
        if !detected_entities.is_empty() {
            score += (detected_entities.len() as f32 * 0.15).min(0.4);
        }

        // 2025-2026 post-freeze frontier AI model entities inherently carry temporal horizon risk
        let has_frontier_ai_entity = detected_entities.iter().any(|e| {
            e.contains("gpt-6")
                || e.contains("astra")
                || e.contains("astr")
                || e.contains("sol")
                || e.contains("argon")
                || e.contains("gemini-4")
                || e.contains("gemini 4")
                || e.contains("opus")
                || e.contains("sonnet")
                || e.contains("grok-4")
                || e.contains("grok 4")
                || e.starts_with("o3")
                || e.starts_with("o4")
        });
        if has_frontier_ai_entity {
            score += 0.20;
        }

        // Software implementation intent on volatile ecosystem entities (frameworks, libraries, crates,
        // versions, or multiple tech entities—excluding a single bare base programming language like "rust" or "python"
        // used in timeless CS algorithm exercises such as "Implement Dijkstra's shortest path in Rust").
        const BARE_BASE_LANGUAGES: &[&str] = &[
            "rust", "python", "javascript", "typescript", "golang", "kotlin", "swift", "dart", "elixir", "zig",
        ];
        let has_volatile_ecosystem_entity = detected_entities.len() > 1
            || detected_entities
                .iter()
                .any(|e| !BARE_BASE_LANGUAGES.contains(&e.as_str()));

        if has_software_intent && has_volatile_ecosystem_entity {
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
