use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

struct CacheEntry {
    value: Value,
    inserted_at: Instant,
    ttl: Duration,
    hits: AtomicU64,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

pub struct ToolResponseCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
    default_ttl: Duration,
    max_entries: usize,
    total_hits: AtomicU64,
    total_misses: AtomicU64,
}

impl Default for ToolResponseCache {
    fn default() -> Self {
        Self::new(300, 500)
    }
}

impl ToolResponseCache {
    pub fn new(ttl_seconds: u64, max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            default_ttl: Duration::from_secs(ttl_seconds),
            max_entries,
            total_hits: AtomicU64::new(0),
            total_misses: AtomicU64::new(0),
        }
    }

    pub fn compute_key(tool_name: &str, arguments: &Value) -> String {
        let mut hasher = Sha256::new();
        hasher.update(tool_name.as_bytes());
        hasher.update(b":");
        let arg_str = match arguments {
            Value::Object(map) => {
                let mut sorted: Vec<(&String, &Value)> = map.iter().collect();
                sorted.sort_by_key(|(k, _)| *k);
                serde_json::to_string(&sorted).unwrap_or_default()
            }
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        hasher.update(arg_str.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn get(&self, tool_name: &str, arguments: &Value) -> Option<Value> {
        let key = Self::compute_key(tool_name, arguments);

        if let Ok(entries) = self.entries.read() {
            if let Some(entry) = entries.get(&key) {
                if !entry.is_expired() {
                    entry.hits.fetch_add(1, Ordering::Relaxed);
                    self.total_hits.fetch_add(1, Ordering::Relaxed);
                    return Some(entry.value.clone());
                }
            }
        }

        self.total_misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    pub fn put(&self, tool_name: &str, arguments: &Value, value: Value) {
        let key = Self::compute_key(tool_name, arguments);

        if let Ok(mut entries) = self.entries.write() {
            // Evict if at capacity
            if entries.len() >= self.max_entries {
                let expired_keys: Vec<String> = entries
                    .iter()
                    .filter(|(_, v)| v.is_expired())
                    .map(|(k, _)| k.clone())
                    .collect();
                for k in expired_keys {
                    entries.remove(&k);
                }

                if entries.len() >= self.max_entries {
                    // Evict oldest entry
                    if let Some(oldest) = entries
                        .iter()
                        .min_by_key(|(_, v)| v.inserted_at)
                        .map(|(k, _)| k.clone())
                    {
                        entries.remove(&oldest);
                    }
                }
            }

            entries.insert(
                key,
                CacheEntry {
                    value,
                    inserted_at: Instant::now(),
                    ttl: self.default_ttl,
                    hits: AtomicU64::new(0),
                },
            );
        }
    }

    pub fn stats(&self) -> (u64, u64, usize, f32) {
        let hits = self.total_hits.load(Ordering::Relaxed);
        let misses = self.total_misses.load(Ordering::Relaxed);
        let count = self.entries.read().map(|e| e.len()).unwrap_or(0);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        (hits, misses, count, (hit_rate * 10.0).round() / 10.0)
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }
}
