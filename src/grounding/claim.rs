use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimCategory {
    TechnicalApi,
    VersionCompatibility,
    TemporalEvent,
    FactualAssertion,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicClaim {
    pub id: String,
    pub statement: String,
    pub category: ClaimCategory,
}

pub struct ClaimExtractor {
    sentence_split_regex: Regex,
    conversational_filter_regex: Regex,
    api_keyword_regex: Regex,
    version_regex: Regex,
}

impl Default for ClaimExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaimExtractor {
    pub fn new() -> Self {
        Self {
            sentence_split_regex: Regex::new(r"([.!?]+(\s+|$))").unwrap(),
            conversational_filter_regex: Regex::new(r"(?i)^(hello|hi|sure|certainly|i can help|let's|in this guide|as you know|hope this helps|feel free)\b").unwrap(),
            api_keyword_regex: Regex::new(r"(?i)\b(function|method|endpoint|struct|class|parameter|returns?|supports?|implements?|deprecated|released|features?|protocol)\b").unwrap(),
            version_regex: Regex::new(r"(?i)(\b(next\.?js|react|rust|node|python|vue|angular)\s+\d+\b|\bv?\d+(\.\d+)+\b|\bv\d+\b)").unwrap(),
        }
    }

    pub fn extract_claims(&self, text: &str) -> Vec<AtomicClaim> {
        let mut claims = Vec::new();
        let mut idx = 1;
        let mut in_code_fence = false;

        // Split text by lines and sentences, skipping fenced code block interiors
        for line in text.lines() {
            let trimmed_line = line.trim();
            if trimmed_line.starts_with("```") {
                in_code_fence = !in_code_fence;
                continue;
            }
            if in_code_fence || trimmed_line.starts_with('#') || trimmed_line.is_empty() {
                continue;
            }

            let sentences = split_prose_sentences(trimmed_line, &self.sentence_split_regex);
            for sentence in sentences {
                let s = sentence.trim().trim_start_matches(|c: char| c == '-' || c == '*' || c.is_whitespace());
                // Filter short utterances or greetings
                if s.len() < 12 || self.conversational_filter_regex.is_match(s) {
                    continue;
                }

                // Categorize
                let category = if self.version_regex.is_match(s) {
                    ClaimCategory::VersionCompatibility
                } else if self.api_keyword_regex.is_match(s) {
                    ClaimCategory::TechnicalApi
                } else if s.contains("released") || s.contains("since") || s.contains("year") {
                    ClaimCategory::TemporalEvent
                } else {
                    ClaimCategory::FactualAssertion
                };

                claims.push(AtomicClaim {
                    id: format!("claim_{}", idx),
                    statement: s.to_string(),
                    category,
                });
                idx += 1;
            }
        }

        claims
    }

    /// Extracts fenced markdown code blocks `(language_hint, code_body)` from a model response
    /// so code blocks can be audited by `CodeInvariantChecker` alongside prose claim verification.
    pub fn extract_fenced_code_blocks(&self, text: &str) -> Vec<(String, String)> {
        let mut blocks = Vec::new();
        let mut in_fence = false;
        let mut current_lang = String::new();
        let mut current_code = String::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("```") {
                if !in_fence {
                    in_fence = true;
                    current_lang = rest.trim().to_string();
                    current_code.clear();
                } else {
                    in_fence = false;
                    if !current_code.trim().is_empty() {
                        blocks.push((current_lang.clone(), current_code.clone()));
                    }
                    current_lang.clear();
                    current_code.clear();
                }
            } else if in_fence {
                current_code.push_str(line);
                current_code.push('\n');
            }
        }

        blocks
    }
}

/// Splits a prose line into sentences while protecting version decimals (`1.3`, `v2.0`)
/// and common abbreviations (`e.g.`, `i.e.`, `vs.`, `Next.js`, `Node.js`).
fn split_prose_sentences(line: &str, split_regex: &Regex) -> Vec<String> {
    let protected = line
        .replace("e.g. ", "e\u{0000}g\u{0000} ")
        .replace("i.e. ", "i\u{0000}e\u{0000} ")
        .replace("vs. ", "vs\u{0000} ")
        .replace("Next.js", "Next\u{0000}js")
        .replace("next.js", "next\u{0000}js")
        .replace("Node.js", "Node\u{0000}js")
        .replace("node.js", "node\u{0000}js")
        .replace("Vue.js", "Vue\u{0000}js")
        .replace("Three.js", "Three\u{0000}js");

    split_regex
        .split(&protected)
        .map(|s| s.replace('\u{0000}', ".").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

