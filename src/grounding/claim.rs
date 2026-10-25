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
    cushion_prefix_regex: Regex,
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
        static REGEXES: std::sync::OnceLock<(Regex, Regex, Regex, Regex, Regex)> =
            std::sync::OnceLock::new();
        let (s_re, c_re, cf_re, api_re, ver_re) = REGEXES
            .get_or_init(|| {
                (
                    Regex::new(r"([.!?]+(\s+|$))").unwrap(),
                    Regex::new(
                        r"(?i)^(?:(?:certainly|sure|of course|as you know|in this guide|great question|good question)[,!:]?\s+|let'?s\s+(?:just\s+)?)",
                    )
                    .unwrap(),
                    Regex::new(
                        r"(?i)^(hello|hi|sure|certainly|i can help|i'd be happy to help|i would be happy to help|let's get started|hope this helps|feel free)\b",
                    )
                    .unwrap(),
                    Regex::new(r"(?i)\b(function|method|endpoint|struct|class|parameter|returns?|supports?|implements?|deprecated|released|features?|protocol)\b").unwrap(),
                    Regex::new(r"(?i)(\b(next\.?js|react|rust|node|python|vue|angular)\s+\d+\b|\bv?\d+(\.\d+)+\b|\bv\d+\b)").unwrap(),
                )
            })
            .clone();

        Self {
            sentence_split_regex: s_re,
            cushion_prefix_regex: c_re,
            conversational_filter_regex: cf_re,
            api_keyword_regex: api_re,
            version_regex: ver_re,
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
                let raw_s = sentence.trim().trim_start_matches(|c: char| c == '-' || c == '*' || c.is_whitespace());
                let stripped_cow = self.cushion_prefix_regex.replace(raw_s, "");
                let s = stripped_cow.trim();
                // Filter short utterances or pure greetings
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
        .replace("et al. ", "et al\u{0000} ")
        .replace("cf. ", "cf\u{0000} ")
        .replace("Thm. ", "Thm\u{0000} ")
        .replace("Lem. ", "Lem\u{0000} ")
        .replace("Prop. ", "Prop\u{0000} ")
        .replace("Cor. ", "Cor\u{0000} ")
        .replace("Eq. ", "Eq\u{0000} ")
        .replace("Fig. ", "Fig\u{0000} ")
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

