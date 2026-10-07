use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCandidate {
    pub name: String,
    pub description: String,
    pub server_name: String,
    pub parameters: Vec<String>,
    pub estimated_tokens: usize,
    #[serde(default)]
    pub is_pinned: bool,
}

impl ToolCandidate {
    pub fn new(name: &str, description: &str, server_name: &str, parameters: Vec<String>) -> Self {
        let total_chars = name.len() + description.len() + server_name.len() + parameters.iter().map(|p| p.len()).sum::<usize>();
        let estimated_tokens = (total_chars / 4).max(180);
        Self {
            name: name.to_string(),
            description: description.to_string(),
            server_name: server_name.to_string(),
            parameters,
            estimated_tokens,
            is_pinned: false,
        }
    }

    pub fn new_pinned(name: &str, description: &str, server_name: &str, parameters: Vec<String>) -> Self {
        let mut t = Self::new(name, description, server_name, parameters);
        t.is_pinned = true;
        t
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
    intent_dictionary: HashMap<String, Vec<String>>,
    pinned_names: HashSet<String>,
}

impl Default for TfidfToolRouter {
    fn default() -> Self {
        Self::new(0.08)
    }
}

impl TfidfToolRouter {
    pub fn new(default_threshold: f32) -> Self {
        let mut intent_dictionary: HashMap<String, Vec<String>> = HashMap::new();
        let mappings = vec![
            ("compile", vec!["run_command", "terminal_exec", "read_file", "view_file"]),
            ("build", vec!["run_command", "terminal_exec", "read_file", "view_file"]),
            ("cargo", vec!["run_command", "terminal_exec"]),
            ("npm", vec!["run_command", "terminal_exec"]),
            ("make", vec!["run_command", "terminal_exec"]),
            ("error", vec!["read_file", "view_file", "run_command", "mobile_diagnose"]),
            ("failing", vec!["read_file", "view_file", "run_command"]),
            ("crash", vec!["read_file", "view_file", "run_command", "mobile_diagnose"]),
            ("trace", vec!["read_file", "view_file", "run_command", "mobile_inspect_trace"]),
            ("bug", vec!["read_file", "view_file", "run_command"]),
            ("test", vec!["run_command", "read_file"]),
            ("git", vec!["run_command", "git_commit"]),
            ("commit", vec!["run_command", "git_commit"]),
            ("push", vec!["run_command"]),
            ("diff", vec!["run_command", "read_file", "view_file"]),
            ("architecture", vec!["chronofact_verify_code_invariants", "spine_reality_audit", "chronofact_memory_save"]),
            ("invariant", vec!["chronofact_verify_code_invariants", "spine_reality_audit"]),
            ("security", vec!["chronofact_verify_code_invariants", "spine_reality_audit"]),
            ("audit", vec!["chronofact_verify_code_invariants", "spine_reality_audit"]),
            ("temporal", vec!["chronofact_temporal_check"]),
            ("cutoff", vec!["chronofact_temporal_check"]),
            ("freeze", vec!["chronofact_temporal_check"]),
            ("drift", vec!["chronofact_temporal_check"]),
            ("search", vec!["search_web", "web_search", "chronofact_ground_query", "read_url_content"]),
            ("docs", vec!["search_web", "read_url_content", "chronofact_ground_query"]),
            ("documentation", vec!["search_web", "read_url_content", "chronofact_ground_query"]),
            ("ground", vec!["chronofact_ground_query"]),
        ];

        for (intent, target_tools) in mappings {
            intent_dictionary.insert(
                intent.to_string(),
                target_tools.into_iter().map(|s| s.to_string()).collect(),
            );
        }

        Self {
            default_threshold,
            intent_dictionary,
            pinned_names: HashSet::new(),
        }
    }

    pub fn with_pinned_tools(mut self, pinned: &[&str]) -> Self {
        for p in pinned {
            self.pinned_names.insert(p.to_string());
        }
        self
    }

    pub fn pin_tool(&mut self, tool_name: &str) {
        self.pinned_names.insert(tool_name.to_string());
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

        // Detect semantic intent triggers to resolve idiomatic queries with zero unigram overlap
        let mut intent_matched_tools: HashSet<String> = HashSet::new();
        for qt in &query_tokens {
            if let Some(target_list) = self.intent_dictionary.get(qt) {
                for target in target_list {
                    intent_matched_tools.insert(target.clone());
                }
            }
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

        // Score tools with hybrid TF-IDF + Semantic Intent Boost
        let mut scored: Vec<(usize, f32)> = doc_tokens
            .iter()
            .enumerate()
            .map(|(i, tokens)| {
                let mut score = tfidf_cosine_similarity(&query_tokens, tokens, &idf);
                let tool_name = &tools[i].name;
                // If semantic intent matched, boost relevance score
                if intent_matched_tools.contains(tool_name) {
                    score = (score + 0.35).min(1.0);
                }
                (i, score)
            })
            .filter(|(i, score)| {
                let tool = &tools[*i];
                let is_pinned = tool.is_pinned || self.pinned_names.contains(&tool.name);
                is_pinned || *score >= threshold
            })
            .collect();

        // Sort descending by score
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Always-Retain Pinning enforcement: Pinned tools must never be pruned
        let mut selected_indices: Vec<(usize, f32)> = Vec::new();
        let mut added_indices: HashSet<usize> = HashSet::new();

        // 1. Add all pinned tools first
        for (i, tool) in tools.iter().enumerate() {
            if tool.is_pinned || self.pinned_names.contains(&tool.name) {
                let score = scored.iter().find(|(idx, _)| *idx == i).map(|(_, s)| *s).unwrap_or(1.0);
                selected_indices.push((i, score));
                added_indices.insert(i);
            }
        }

        // 2. Fill remaining slots up to top_k with highest scoring tools
        for (i, score) in scored {
            if !added_indices.contains(&i) {
                if selected_indices.len() >= top_k {
                    break;
                }
                selected_indices.push((i, score));
                added_indices.insert(i);
            }
        }

        // 3. Fallback: If nothing was matched at all and no pinned tools present
        if selected_indices.is_empty() {
            for (i, _) in tools.iter().enumerate().take(2.min(tools.len())) {
                selected_indices.push((i, 0.5));
            }
        }

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
