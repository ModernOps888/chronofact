use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCandidate {
    pub name: String,
    pub description: String,
    pub server_name: String,
    pub parameters: Vec<String>,
    pub estimated_tokens: usize,
}

impl ToolCandidate {
    pub fn new(name: &str, description: &str, server_name: &str, parameters: Vec<String>) -> Self {
        // Approximate token calculation: ~4 chars per token + schema JSON overhead
        let total_chars = name.len() + description.len() + server_name.len() + parameters.iter().map(|p| p.len()).sum::<usize>();
        let estimated_tokens = (total_chars / 4).max(180); // typical minimal MCP tool schema is ~180-250 tokens
        Self {
            name: name.to_string(),
            description: description.to_string(),
            server_name: server_name.to_string(),
            parameters,
            estimated_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredTool {
    pub tool: ToolCandidate,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingResult {
    pub query: String,
    pub selected_tools: Vec<ScoredTool>,
    pub total_tools: usize,
    pub pruned_tools: usize,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub tokens_saved: usize,
    pub savings_percentage: f32,
}

pub struct TfidfToolRouter {
    default_threshold: f32,
}

impl Default for TfidfToolRouter {
    fn default() -> Self {
        Self::new(0.08)
    }
}

impl TfidfToolRouter {
    pub fn new(default_threshold: f32) -> Self {
        Self { default_threshold }
    }

    pub fn route(
        &self,
        query: &str,
        tools: &[ToolCandidate],
        top_k: usize,
        min_threshold: Option<f32>,
    ) -> RoutingResult {
        let threshold = min_threshold.unwrap_or(self.default_threshold);
        let total_tokens: usize = tools.iter().map(|t| t.estimated_tokens).sum();

        if tools.is_empty() || query.trim().is_empty() {
            return RoutingResult {
                query: query.to_string(),
                selected_tools: tools.iter().map(|t| ScoredTool { tool: t.clone(), relevance_score: 1.0 }).collect(),
                total_tools: tools.len(),
                pruned_tools: 0,
                tokens_before: total_tokens,
                tokens_after: total_tokens,
                tokens_saved: 0,
                savings_percentage: 0.0,
            };
        }

        let query_tokens = tokenize(query);
        if query_tokens.is_empty() {
            let selected: Vec<ScoredTool> = tools.iter().take(top_k).map(|t| ScoredTool { tool: t.clone(), relevance_score: 0.5 }).collect();
            let after: usize = selected.iter().map(|s| s.tool.estimated_tokens).sum();
            let saved = total_tokens.saturating_sub(after);
            let pct = if total_tokens > 0 { (saved as f32 / total_tokens as f32) * 100.0 } else { 0.0 };
            return RoutingResult {
                query: query.to_string(),
                selected_tools: selected,
                total_tools: tools.len(),
                pruned_tools: tools.len().saturating_sub(top_k),
                tokens_before: total_tokens,
                tokens_after: after,
                tokens_saved: saved,
                savings_percentage: pct,
            };
        }

        // Build document tokens per tool
        let mut doc_tokens: Vec<Vec<String>> = Vec::new();
        for tool in tools {
            let mut text = format!("{} {} {}", tool.name, tool.server_name, tool.description);
            for param in &tool.parameters {
                text.push(' ');
                text.push_str(param);
            }
            doc_tokens.push(tokenize(&text));
        }

        // Calculate IDF across the toolset
        let total_docs = doc_tokens.len() as f32;
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        for tokens in &doc_tokens {
            let unique: HashSet<&String> = tokens.iter().collect();
            for token in unique {
                *doc_freq.entry(token.clone()).or_insert(0) += 1;
            }
        }

        let idf: HashMap<String, f32> = doc_freq
            .iter()
            .map(|(token, df)| {
                let idf_val = (total_docs / (*df as f32 + 1.0)).ln() + 1.0;
                (token.clone(), idf_val)
            })
            .collect();

        // Score tools
        let mut scored: Vec<(usize, f32)> = doc_tokens
            .iter()
            .enumerate()
            .map(|(i, tokens)| {
                let score = tfidf_cosine_similarity(&query_tokens, tokens, &idf);
                (i, score)
            })
            .filter(|(_, score)| *score >= threshold)
            .collect();

        // Sort descending by score
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // If nothing passed threshold, retain top 2 tools to ensure fallback
        let selected_indices: Vec<(usize, f32)> = if scored.is_empty() {
            let mut fallback: Vec<(usize, f32)> = doc_tokens
                .iter()
                .enumerate()
                .map(|(i, tokens)| (i, tfidf_cosine_similarity(&query_tokens, tokens, &idf)))
                .collect();
            fallback.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            fallback.into_iter().take(2.min(tools.len())).collect()
        } else {
            scored.into_iter().take(top_k).collect()
        };

        let selected_tools: Vec<ScoredTool> = selected_indices
            .into_iter()
            .map(|(i, score)| ScoredTool {
                tool: tools[i].clone(),
                relevance_score: (score * 100.0).round() / 100.0,
            })
            .collect();

        let tokens_after: usize = selected_tools.iter().map(|s| s.tool.estimated_tokens).sum();
        let tokens_saved = total_tokens.saturating_sub(tokens_after);
        let savings_percentage = if total_tokens > 0 {
            (tokens_saved as f32 / total_tokens as f32) * 100.0
        } else {
            0.0
        };

        let pruned_count = tools.len().saturating_sub(selected_tools.len());

        RoutingResult {
            query: query.to_string(),
            selected_tools,
            total_tools: tools.len(),
            pruned_tools: pruned_count,
            tokens_before: total_tokens,
            tokens_after,
            tokens_saved,
            savings_percentage: (savings_percentage * 10.0).round() / 10.0,
        }
    }
}

fn tokenize(text: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "can", "to", "of", "in", "for",
        "on", "with", "at", "by", "from", "as", "into", "through", "during", "before", "after",
        "above", "below", "between", "out", "off", "over", "under", "again", "further", "then",
        "once", "here", "there", "when", "where", "why", "how", "all", "both", "each", "few",
        "more", "most", "other", "some", "such", "no", "not", "only", "own", "same", "so",
        "than", "too", "very", "just", "or", "and", "but", "if", "this", "that", "these",
        "those", "it", "its", "please", "me", "my", "we", "our", "you", "your",
    ];

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 1 && !STOPWORDS.contains(s))
        .map(|s| s.to_string())
        .collect()
}

fn term_freq(tokens: &[String]) -> HashMap<String, f32> {
    let mut tf: HashMap<String, f32> = HashMap::new();
    let total = tokens.len() as f32;
    if total == 0.0 {
        return tf;
    }
    for token in tokens {
        *tf.entry(token.clone()).or_insert(0.0) += 1.0;
    }
    for val in tf.values_mut() {
        *val /= total;
    }
    tf
}

fn tfidf_cosine_similarity(
    query_tokens: &[String],
    doc_tokens: &[String],
    idf: &HashMap<String, f32>,
) -> f32 {
    let q_tf = term_freq(query_tokens);
    let d_tf = term_freq(doc_tokens);

    let mut dot_product = 0.0;
    let mut q_norm_sq = 0.0;
    let mut d_norm_sq = 0.0;

    for (term, q_val) in &q_tf {
        let idf_val = idf.get(term).copied().unwrap_or(1.0);
        let q_weight = q_val * idf_val;
        q_norm_sq += q_weight * q_weight;

        if let Some(d_val) = d_tf.get(term) {
            let d_weight = d_val * idf_val;
            dot_product += q_weight * d_weight;
        }
    }

    for (term, d_val) in &d_tf {
        let idf_val = idf.get(term).copied().unwrap_or(1.0);
        let d_weight = d_val * idf_val;
        d_norm_sq += d_weight * d_weight;
    }

    let q_norm = q_norm_sq.sqrt();
    let d_norm = d_norm_sq.sqrt();

    if q_norm == 0.0 || d_norm == 0.0 {
        0.0
    } else {
        dot_product / (q_norm * d_norm)
    }
}
