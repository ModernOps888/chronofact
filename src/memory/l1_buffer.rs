use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user", "assistant", "system"
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct WorkingBuffer {
    max_turns: usize,
    history: VecDeque<ChatMessage>,
}

impl WorkingBuffer {
    pub fn new(max_turns: usize) -> Self {
        Self {
            max_turns,
            history: VecDeque::new(),
        }
    }

    pub fn push(&mut self, role: &str, content: &str) {
        if self.max_turns == 0 {
            return;
        }
        if self.history.len() >= self.max_turns * 2 {
            self.history.pop_front();
        }
        self.history.push_back(ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
        });
    }

    pub fn get_messages(&self) -> Vec<ChatMessage> {
        self.history.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}
