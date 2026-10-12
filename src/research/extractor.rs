use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceChunk {
    pub id: String,            // e.g. "SRC-1"
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    pub content: String,
    #[serde(default)]
    pub integrity_hash: String,
    #[serde(default)]
    pub is_sanitized: bool,
}

pub struct ContentExtractor;

impl ContentExtractor {
    pub fn clean_text(raw_text: &str) -> String {
        let text = raw_text.replace('\r', " ");
        // Collapse consecutive whitespace
        let words: Vec<&str> = text.split_whitespace().collect();
        words.join(" ")
    }

    pub fn chunk_text(text: &str, max_words: usize) -> Vec<String> {
        if max_words == 0 {
            return Vec::new();
        }
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        for chunk in words.chunks(max_words) {
            chunks.push(chunk.join(" "));
        }
        chunks
    }
}
