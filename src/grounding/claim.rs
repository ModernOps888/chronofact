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

        // Split text by lines and sentences
        for line in text.lines() {
            let trimmed_line = line.trim();
            // Skip headers or markdown code fences from direct raw sentence splitting
            if trimmed_line.starts_with('#') || trimmed_line.starts_with("```") || trimmed_line.is_empty() {
                continue;
            }

            let sentences: Vec<&str> = self.sentence_split_regex.split(trimmed_line).collect();
            for sentence in sentences {
                let s = sentence.trim();
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
}
