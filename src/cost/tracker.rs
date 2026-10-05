use serde::{Deserialize, Serialize};
use std::sync::RwLock;

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
    total_queries: RwLock<u64>,
    total_pruned_tools: RwLock<u64>,
    total_tokens_saved: RwLock<u64>,
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
            total_queries: RwLock::new(0),
            total_pruned_tools: RwLock::new(0),
            total_tokens_saved: RwLock::new(0),
            input_price_per_token,
        }
    }

    pub fn record_savings(&self, pruned_count: usize, tokens_saved: usize) {
        if let Ok(mut q) = self.total_queries.write() {
            *q += 1;
        }
        if let Ok(mut p) = self.total_pruned_tools.write() {
            *p += pruned_count as u64;
        }
        if let Ok(mut s) = self.total_tokens_saved.write() {
            *s += tokens_saved as u64;
        }
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
        let mem_queries = self.total_queries.read().map(|q| *q).unwrap_or(0);
        let mem_pruned = self.total_pruned_tools.read().map(|p| *p).unwrap_or(0);
        let mem_saved = self.total_tokens_saved.read().map(|s| *s).unwrap_or(0);

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
            CHRONOFACT_EPIDEMIOLOGY_VERSION: 1.0.0\n\
            CACHE_COMPATIBILITY: ANTHROPIC_EPHEMERAL_5MIN_V1\n\
            INVARIANT_BLOCK: IMMUTABLE\n\
            </prompt_cache_anchor>\n",
            project_id
        )
    }
}
