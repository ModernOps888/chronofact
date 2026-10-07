use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostMetricsSummary {
    pub total_queries_optimized: u64,
    pub total_tools_pruned: u64,
    pub total_tokens_saved: u64,
    pub estimated_usd_saved: f64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate_pct: f32,
    pub average_tokens_saved_per_turn: f32,
    pub active_cache_entries: usize,
    pub prompt_caching_aligned: bool,
}

pub struct CostTracker {
    total_queries: AtomicU64,
    total_pruned_tools: AtomicU64,
    total_tokens_saved: AtomicU64,
    // Input cost benchmark: $3.00 per 1M tokens ($0.000003/token)
    input_price_per_token: f64,
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new(0.000003) // $3.00 per million tokens (Claude Sonnet / GPT-4o / GPT-6 baseline)
    }
}

impl CostTracker {
    pub fn new(input_price_per_token: f64) -> Self {
        Self {
            total_queries: AtomicU64::new(0),
            total_pruned_tools: AtomicU64::new(0),
            total_tokens_saved: AtomicU64::new(0),
            input_price_per_token,
        }
    }

    pub fn record_savings(&self, pruned_count: usize, tokens_saved: usize) {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        self.total_pruned_tools.fetch_add(pruned_count as u64, Ordering::Relaxed);
        self.total_tokens_saved.fetch_add(tokens_saved as u64, Ordering::Relaxed);
    }

    pub fn get_metrics(&self, cache_hits: u64, cache_misses: u64, active_cache_entries: usize) -> CostMetricsSummary {
        self.get_metrics_with_db((0, 0, 0, 0.0), cache_hits, cache_misses, active_cache_entries)
    }

    pub fn get_metrics_with_db(
        &self,
        db_totals: (u64, u64, u64, f64),
        cache_hits: u64,
        cache_misses: u64,
        active_cache_entries: usize,
    ) -> CostMetricsSummary {
        let (db_queries, db_pruned, db_saved_tokens, db_usd) = db_totals;
        let mem_queries = self.total_queries.load(Ordering::Relaxed);
        let mem_pruned = self.total_pruned_tools.load(Ordering::Relaxed);
        let mem_saved = self.total_tokens_saved.load(Ordering::Relaxed);

        let queries = db_queries.max(mem_queries);
        let pruned = db_pruned.max(mem_pruned);
        let saved_tokens = db_saved_tokens.max(mem_saved);

        let total_cache_attempts = cache_hits + cache_misses;
        let cache_hit_rate = if total_cache_attempts > 0 {
            (cache_hits as f32 / total_cache_attempts as f32) * 100.0
        } else {
            0.0
        };

        let avg_saved = if queries > 0 {
            saved_tokens as f32 / queries as f32
        } else {
            0.0
        };

        let usd_saved = if db_usd > 0.0 {
            (db_usd * 10000.0).round() / 10000.0
        } else {
            (saved_tokens as f64 * self.input_price_per_token * 10000.0).round() / 10000.0
        };

        CostMetricsSummary {
            total_queries_optimized: queries,
            total_tools_pruned: pruned,
            total_tokens_saved: saved_tokens,
            estimated_usd_saved: usd_saved,
            cache_hits,
            cache_misses,
            cache_hit_rate_pct: (cache_hit_rate * 10.0).round() / 10.0,
            average_tokens_saved_per_turn: (avg_saved * 10.0).round() / 10.0,
            active_cache_entries,
            prompt_caching_aligned: true,
        }
    }

    /// Generates a deterministic static prompt prefix designed for 100% provider prompt caching hits
    pub fn generate_cache_aligned_prefix(project_id: &str) -> String {
        format!(
            "<prompt_cache_anchor project=\"{}\" tier=\"L3_INVARIANT_STATIC\">\n\
            CHRONOFACT_EPISTEMIC_VERSION: 1.0.0\n\
            CACHE_COMPATIBILITY: ANTHROPIC_EPHEMERAL_5MIN_V1\n\
            INVARIANT_BLOCK: IMMUTABLE\n\
            </prompt_cache_anchor>\n",
            project_id
        )
    }

    /// Isolates static system instructions, pinned tool schemas, and project invariant anchors into an
    /// immutable static cache prefix (matching upstream Anthropic / OpenAI prompt caching requirements),
    /// strictly separating it from dynamic temporal anchors, retrieved real-time evidence, and user queries.
    /// Guarantees that static prefixes clear the minimum provider prompt caching floor (1,024 tokens)
    /// by deterministically padding standardized byte-stable project context rules when needed.
    pub fn build_cache_isolated_prompt(
        project_id: &str,
        static_system_instructions: &str,
        static_schemas_json: &str,
        temporal_anchor_header: &str,
        dynamic_retrieved_evidence: &str,
        user_query: &str,
    ) -> CacheOptimizedPrompt {
        Self::assemble_cache_aligned_prompt_with_mode(
            project_id,
            static_system_instructions,
            static_schemas_json,
            temporal_anchor_header,
            dynamic_retrieved_evidence,
            user_query,
            SessionMode::InteractiveMultiTurn,
        )
    }

    /// Assembles an isolated prompt with explicit SessionMode support (MultiTurn vs StatelessSingleShot).
    pub fn assemble_cache_aligned_prompt_with_mode(
        project_id: &str,
        static_system_instructions: &str,
        static_schemas_json: &str,
        temporal_anchor_header: &str,
        dynamic_retrieved_evidence: &str,
        user_query: &str,
        session_mode: SessionMode,
    ) -> CacheOptimizedPrompt {
        let anchor = Self::generate_cache_aligned_prefix(project_id);
        
        let mut static_cache_prefix = format!(
            "{}\n\
             [STATIC CACHE PREFIX: System Prompt + Pinned Schemas + Cache Anchor]\n\
             {}\n\n\
             [AVAILABLE_TOOLS_SCHEMAS]\n\
             {}",
            anchor.trim(),
            static_system_instructions.trim(),
            static_schemas_json.trim()
        );

        let current_tokens = static_cache_prefix.len() / 4;
        if session_mode == SessionMode::InteractiveMultiTurn && current_tokens < PROMPT_CACHE_MINIMUM_TOKEN_FLOOR {
            let deficit = PROMPT_CACHE_MINIMUM_TOKEN_FLOOR - current_tokens;
            let padding = generate_standardized_cache_context_rules(project_id, deficit);
            static_cache_prefix.push_str(&padding);
        }

        let dynamic_context_suffix = format!(
            "[DYNAMIC SUFFIX: Temporal Anchor Header + Retrieved Evidence + User Query]\n\
             {}\n\n\
             [RETRIEVED_EPISTEMIC_GROUNDING]\n\
             {}\n\n\
             [USER_QUERY]\n\
             {}",
            temporal_anchor_header.trim(),
            dynamic_retrieved_evidence.trim(),
            user_query.trim()
        );

        let cache_boundary_marker = "\n<!-- CACHE_BOUNDARY_EPHEMERAL -->\n".to_string();
        let full_assembled_prompt = format!("{}{}{}", static_cache_prefix, cache_boundary_marker, dynamic_context_suffix);

        let static_prefix_estimated_tokens = (static_cache_prefix.len() / 4).max(
            if session_mode == SessionMode::InteractiveMultiTurn {
                PROMPT_CACHE_MINIMUM_TOKEN_FLOOR
            } else {
                1
            }
        );
        let dynamic_suffix_estimated_tokens = (dynamic_context_suffix.len() / 4).max(1);

        CacheOptimizedPrompt {
            project_id: project_id.to_string(),
            static_cache_prefix,
            dynamic_context_suffix,
            cache_boundary_marker,
            full_assembled_prompt,
            static_prefix_estimated_tokens,
            dynamic_suffix_estimated_tokens,
            cache_aligned: session_mode == SessionMode::InteractiveMultiTurn,
            session_mode,
        }
    }
}

/// Execution mode governing whether prompt cache padding is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionMode {
    /// Enforces 1,088-token cache floor (1,024 + 64-token BPE discrepancy safety margin) for multi-turn prompt caching.
    InteractiveMultiTurn,
    /// Stateless one-shot or headless CI execution; skips artificial padding to minimize token consumption.
    StatelessSingleShot,
}

impl Default for SessionMode {
    fn default() -> Self {
        SessionMode::InteractiveMultiTurn
    }
}

/// Minimum token threshold required by frontier providers (Anthropic Claude 3.5/3.7/Opus/Sonnet 5.5 and OpenAI GPT-4o/o1/o3)
/// before server-side prompt caching engages. Padded to 1,088 tokens (1,024 base floor + 64-token safety margin)
/// to guarantee crossing the threshold across divergent BPE tokenizers (OpenAI o200k/cl100k vs Anthropic Claude BPE).
pub const PROMPT_CACHE_MINIMUM_TOKEN_FLOOR: usize = 1088;

/// Generates byte-stable standardized project context rules to guarantee that the static cache prefix
/// clears the minimum provider prompt caching floor (1,088 tokens with 64-token safety buffer).
pub fn generate_standardized_cache_context_rules(project_id: &str, deficit_tokens: usize) -> String {
    let mut buffer = String::new();
    buffer.push_str(&format!(
        "\n\n[STANDARDIZED_PROJECT_CACHE_CONTEXT]\n\
         PROJECT_IDENTIFIER: {}\n\
         EPISTEMIC_TIER: L3_INVARIANT_STATIC\n\
         PROVIDER_CACHE_POLICY: MIN_FLOOR_1088_TOKENS_WITH_64_SAFETY_BUFFER\n\
         SYSTEM_INVARIANTS:\n\
         - INVARIANT_1: All factual claims must be anchored against current epistemic verification.\n\
         - INVARIANT_2: Prohibit unearned concessions and sycophantic capitulation to invalidated user claims.\n\
         - INVARIANT_3: Enforce strict byte-level prefix stability across multi-turn interactions.\n\
         - INVARIANT_4: Maintain deterministic schema definitions to prevent mid-session cache invalidation.\n\
         - INVARIANT_5: Isolate dynamic temporal markers into ephemeral prompt suffixes.\n\
         - INVARIANT_6: Tool invocations must validate parameter types and bounds before execution.\n\
         - INVARIANT_7: Workspace modifications must be tracked with cryptographically verifiable checksums.\n\
         - INVARIANT_8: Reject hallucinated model versions, retired SDK patterns, and frozen cutoff dates.\n",
        project_id
    ));

    let base_estimated = buffer.len() / 4;
    if base_estimated < deficit_tokens {
        let additional_needed = deficit_tokens - base_estimated;
        let blocks_needed = (additional_needed / 25).max(1);
        for i in 0..blocks_needed {
            buffer.push_str(&format!(
                "- REPEATABLE_ANCHOR_CLAUSE_{:03}: Static project context verification block for '{}' maintaining prompt cache stability.\n",
                i + 1, project_id
            ));
        }
    }
    buffer
}

/// Representation of a strictly isolated prompt preserving byte-level prompt cache prefixes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheOptimizedPrompt {
    pub project_id: String,
    pub static_cache_prefix: String,
    pub dynamic_context_suffix: String,
    pub cache_boundary_marker: String,
    pub full_assembled_prompt: String,
    pub static_prefix_estimated_tokens: usize,
    pub dynamic_suffix_estimated_tokens: usize,
    pub cache_aligned: bool,
    pub session_mode: SessionMode,
}

