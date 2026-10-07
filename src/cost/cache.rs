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
    associated_path: Option<String>,
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
        Self::compute_key_with_fs_metadata(tool_name, arguments)
    }

    /// Computes a cache key incorporating filesystem metadata (mtime and size) when target arguments reference local files.
    /// This prevents stale cache hits on read_file/view_file when underlying files mutate on disk.
    pub fn compute_key_with_fs_metadata(tool_name: &str, arguments: &Value) -> String {
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

        // Filesystem State Awareness:
        // If the tool arguments contain a target file path (e.g. read_file, view_file),
        // incorporate canonical_path, mtime (nanoseconds), and file_size into the hash:
        // SHA-256(tool_name + canonical_path + mtime + file_size).
        if let Some(val) = Self::extract_path(arguments) {
            let canon_path = std::path::Path::new(&val)
                .canonicalize()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_else(|| val.replace('\\', "/"));

            if let Ok(meta) = std::fs::metadata(&val) {
                let mtime = meta.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let len = meta.len();
                hasher.update(format!(":canonical_path={}:mtime={}:file_size={}", canon_path, mtime, len).as_bytes());
            } else {
                hasher.update(format!(":canonical_path={}", canon_path).as_bytes());
            }
        }

        format!("{:x}", hasher.finalize())
    }

    /// Extracts a file path from arguments if present under standard file parameter keys.
    pub fn extract_path(arguments: &Value) -> Option<String> {
        let file_path_keys = ["path", "AbsolutePath", "file_path", "TargetFile", "file", "target_file", "filename"];
        if let Value::Object(map) = arguments {
            for key in &file_path_keys {
                if let Some(val) = map.get(*key).and_then(|v| v.as_str()) {
                    return Some(val.to_string());
                }
            }
        }
        None
    }

    /// Invalidates all cached entries referencing a specific file path or filename.
    pub fn invalidate_path(&self, path: &str) {
        if let Ok(mut entries) = self.entries.write() {
            let path_clean = path.replace('\\', "/").to_lowercase();
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_lowercase();

            let keys_to_remove: Vec<String> = entries
                .iter()
                .filter(|(_, entry)| {
                    if let Some(ref p) = entry.associated_path {
                        let p_clean = p.replace('\\', "/").to_lowercase();
                        if p_clean.contains(&path_clean) || path_clean.contains(&p_clean) || p_clean.ends_with(&file_name) {
                            return true;
                        }
                    }
                    let entry_str = entry.value.to_string().to_lowercase();
                    entry_str.contains(&path_clean) || entry_str.contains(&file_name)
                })
                .map(|(k, _)| k.clone())
                .collect();
            for k in keys_to_remove {
                entries.remove(&k);
            }
        }
    }

    /// Invalidates all cached entries that reference any local filesystem path.
    /// Used when mutating operations (e.g. build commands or terminal execution) may alter disk state.
    pub fn invalidate_filesystem_entries(&self) {
        if let Ok(mut entries) = self.entries.write() {
            let keys_to_remove: Vec<String> = entries
                .iter()
                .filter(|(_, entry)| entry.associated_path.is_some())
                .map(|(k, _)| k.clone())
                .collect();
            for k in keys_to_remove {
                entries.remove(&k);
            }
        }
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
        let associated_path = Self::extract_path(arguments);

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
                    associated_path,
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
