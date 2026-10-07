//! Calibrated Memory Thresholds & Adaptive Density Windowing
//!
//! Replaces rigid static similarity cutoffs (e.g. 0.72) with provider-calibrated baselines
//! and relative distribution statistics (mean + k*stddev) to prevent over-filtering or
//! context leakage across disparate embedding spaces (OpenAI, BGE, Cohere, FastEmbed).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingProvider {
    OpenAiTextEmbedding3, // Dense, normalized, typical baseline ~0.70
    BgeLarge,             // Sharp distribution, typical baseline ~0.55
    CohereV3,             // Asymmetric cosine, typical baseline ~0.45
    FastEmbedMiniLm,      // Local quantized, typical baseline ~0.60
    LexicalTokenOverlap,  // Discrete intersection, typical baseline ~0.15
    Custom,
}

impl EmbeddingProvider {
    pub fn default_floor_threshold(&self) -> f32 {
        match self {
            Self::OpenAiTextEmbedding3 => 0.70,
            Self::BgeLarge => 0.55,
            Self::CohereV3 => 0.45,
            Self::FastEmbedMiniLm => 0.60,
            Self::LexicalTokenOverlap => 0.15,
            Self::Custom => 0.50,
        }
    }
}

pub struct MemoryCalibrator;

impl MemoryCalibrator {
    /// Computes an adaptive relative threshold from candidate similarity scores.
    /// Uses sample mean mu and standard deviation sigma: threshold = max(floor * 0.8, mu + multiplier * sigma)
    pub fn calculate_adaptive_threshold(
        scores: &[f32],
        provider: EmbeddingProvider,
        multiplier: f32,
    ) -> f32 {
        let floor = provider.default_floor_threshold();
        if scores.is_empty() {
            return floor;
        }

        let sum: f32 = scores.iter().sum();
        let mean = sum / scores.len() as f32;

        let variance: f32 = scores
            .iter()
            .map(|s| {
                let diff = s - mean;
                diff * diff
            })
            .sum::<f32>()
            / scores.len() as f32;

        let stddev = variance.sqrt();
        let relative_threshold = mean + multiplier * stddev;

        // Strict Two-Tier Gate: Never drop below the provider's absolute baseline floor, nor exceed ceiling (0.95)
        relative_threshold.max(floor).min(0.95)
    }

    /// Filters and ranks candidate memory entities using a two-tier gate:
    /// Tier 1: Candidate score must meet the provider's absolute baseline floor.
    /// Tier 2: Qualified candidates must meet the adaptive distribution threshold (mean + multiplier * stddev).
    /// If all candidate scores reside in a low-similarity noise cluster (e.g. 0.10-0.22), returns empty to prevent context pollution.
    pub fn filter_adaptive<T: Clone>(
        items: Vec<(T, f32)>,
        provider: EmbeddingProvider,
        multiplier: f32,
        max_top_k: usize,
    ) -> Vec<(T, f32)> {
        if items.is_empty() {
            return Vec::new();
        }

        let floor = provider.default_floor_threshold();
        // Tier 1 Gate: Eliminate low-similarity cluster noise below absolute floor
        let qualified: Vec<(T, f32)> = items.into_iter().filter(|(_, s)| *s >= floor).collect();
        if qualified.is_empty() {
            return Vec::new();
        }

        let scores: Vec<f32> = qualified.iter().map(|(_, s)| *s).collect();
        let adaptive_threshold = Self::calculate_adaptive_threshold(&scores, provider, multiplier);

        let mut filtered: Vec<(T, f32)> = qualified
            .into_iter()
            .filter(|(_, s)| *s >= adaptive_threshold)
            .collect();

        filtered.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        filtered.truncate(max_top_k);
        filtered
    }

    /// Computes a percentile-based rank threshold from candidate similarity scores.
    /// E.g. percentile = 0.70 means only scores in the top 30% are retained.
    pub fn calculate_percentile_threshold(scores: &[f32], percentile: f32) -> f32 {
        if scores.is_empty() {
            return 0.5;
        }
        let mut sorted = scores.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let p_clamped = percentile.clamp(0.0, 1.0);
        let index = ((sorted.len() as f32 - 1.0) * p_clamped).round() as usize;
        sorted[index]
    }

    /// Filters and ranks candidate memory entities using percentile-based ranking constrained by an absolute baseline floor.
    /// Low-similarity candidates below absolute_floor are purged before percentile ranking.
    pub fn filter_by_percentile_with_floor<T: Clone>(
        items: Vec<(T, f32)>,
        percentile: f32,
        absolute_floor: f32,
        max_top_k: usize,
    ) -> Vec<(T, f32)> {
        if items.is_empty() {
            return Vec::new();
        }

        // Tier 1 Gate: Drop low-similarity cluster noise below absolute floor
        let qualified: Vec<(T, f32)> = items
            .into_iter()
            .filter(|(_, s)| *s >= absolute_floor)
            .collect();

        if qualified.is_empty() {
            return Vec::new();
        }

        let scores: Vec<f32> = qualified.iter().map(|(_, s)| *s).collect();
        let rank_threshold = Self::calculate_percentile_threshold(&scores, percentile);

        let mut filtered: Vec<(T, f32)> = qualified
            .into_iter()
            .filter(|(_, s)| *s >= rank_threshold)
            .collect();

        filtered.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        filtered.truncate(max_top_k);
        filtered
    }

    /// Filters and ranks candidate memory entities using percentile-based ranking with a baseline floor of 0.35.
    pub fn filter_by_percentile<T: Clone>(
        items: Vec<(T, f32)>,
        percentile: f32,
        max_top_k: usize,
    ) -> Vec<(T, f32)> {
        Self::filter_by_percentile_with_floor(items, percentile, 0.35, max_top_k)
    }
}

/// Provider-calibrated embedding baseline profile for dynamic threshold comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProfile {
    pub provider: EmbeddingProvider,
    pub calibrated_threshold: f32,
}

impl EmbeddingProfile {
    pub fn new(provider: EmbeddingProvider) -> Self {
        Self {
            calibrated_threshold: provider.default_floor_threshold(),
            provider,
        }
    }

    pub fn with_threshold(provider: EmbeddingProvider, threshold: f32) -> Self {
        Self {
            provider,
            calibrated_threshold: threshold,
        }
    }
}

/// Evaluates whether a memory similarity score exceeds the embedding model's calibrated baseline.
pub fn is_memory_relevant(score: f32, model_baseline: &EmbeddingProfile) -> bool {
    score >= model_baseline.calibrated_threshold
}

