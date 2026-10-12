//! Keyless release/EOL grounding from endoflife.date (https://endoflife.date/docs/api).
//!
//! Answers "what is the latest Python/Node/Go/Django/... and until when is it supported" from a
//! public JSON API with no key and no HTML scraping. The product list is fetched from the API
//! itself, so nothing here is hand-curated beyond a few spelling aliases.

use super::extractor::SourceChunk;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const LIST_TTL: Duration = Duration::from_secs(24 * 3600);
const PRODUCT_TTL: Duration = Duration::from_secs(3600);
const MAX_PRODUCTS: usize = 2;
const MAX_CYCLES: usize = 3;

pub struct EolClient {
    client: reqwest::Client,
    list: RwLock<Option<(HashSet<String>, Instant)>>,
    cache: RwLock<HashMap<String, (SourceChunk, Instant)>>,
}

impl Default for EolClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Query shape that asks about releases/support rather than something else that happens to
/// contain a product name ("go ahead", "java coffee").
pub fn has_release_intent(lower: &str) -> bool {
    const CUES: &[&str] = &[
        "latest", "newest", "current", "version", "release", "released", "eol", "end of life",
        "end-of-life", "supported", "support", "lts", "upgrade", "deprecated", "maintained",
    ];
    CUES.iter().any(|c| lower.contains(c))
}

/// Slug candidates named by the query (before checking them against the live product list).
pub fn slug_candidates(lower: &str) -> Vec<String> {
    let toks: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '.')))
        .map(|t| t.trim_matches(|c: char| matches!(c, '-' | '.')))
        .filter(|t| !t.is_empty())
        .collect();
    let mut out: Vec<String> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let (slug, aliased) = match *t {
            "node" | "node.js" => ("nodejs", true),
            "golang" => ("go", true),
            "k8s" => ("kubernetes", true),
            "postgres" => ("postgresql", true),
            ".net" | "dotnet" => ("dotnet", true),
            other => (other, false),
        };
        // "rust crate", "python package": the word qualifies a package, it is not asking about the language.
        let qualifies_package = toks.get(i + 1).map_or(false, |n| {
            matches!(*n, "crate" | "crates" | "package" | "packages" | "library" | "lib" | "module" | "modules")
        });
        if qualifies_package {
            continue;
        }
        // Very short slugs ("go") are ordinary words unless aliased or followed by a version number.
        let next_is_version = toks.get(i + 1).map_or(false, |n| n.chars().next().map_or(false, |c| c.is_ascii_digit()));
        if slug.len() < 4 && !aliased && !next_is_version {
            continue;
        }
        if !out.iter().any(|s| s == slug) {
            out.push(slug.to_string());
        }
    }
    out
}

impl EolClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("ChronoFact/0.1.0 (https://github.com/ModernOps888/chronofact)")
            .build()
            .unwrap_or_default();
        Self { client, list: RwLock::new(None), cache: RwLock::new(HashMap::new()) }
    }

    async fn products(&self) -> Option<HashSet<String>> {
        if let Some((set, at)) = self.list.read().await.as_ref() {
            if at.elapsed() < LIST_TTL {
                return Some(set.clone());
            }
        }
        let resp = self.client.get("https://endoflife.date/api/all.json").send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let v: Value = resp.json().await.ok()?;
        let set: HashSet<String> = v.as_array()?.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
        if set.is_empty() {
            return None;
        }
        *self.list.write().await = Some((set.clone(), Instant::now()));
        Some(set)
    }

    pub async fn ground_query(&self, query: &str) -> Vec<SourceChunk> {
        let lower = query.to_lowercase();
        if !has_release_intent(&lower) {
            return Vec::new();
        }
        let cands = slug_candidates(&lower);
        if cands.is_empty() {
            return Vec::new();
        }
        let Some(products) = self.products().await else { return Vec::new() };
        let mut out = Vec::new();
        for slug in cands.into_iter().filter(|s| products.contains(s)).take(MAX_PRODUCTS) {
            if let Some(c) = self.product_chunk(&slug).await {
                out.push(c);
            }
        }
        out
    }

    async fn product_chunk(&self, slug: &str) -> Option<SourceChunk> {
        if let Some((c, at)) = self.cache.read().await.get(slug) {
            if at.elapsed() < PRODUCT_TTL {
                return Some(c.clone());
            }
        }
        let resp = self.client.get(format!("https://endoflife.date/api/{}.json", slug)).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let v: Value = resp.json().await.ok()?;
        let content = format_cycles(slug, v.as_array()?)?;
        let chunk = SourceChunk {
            id: format!("EOL-{}", slug.to_uppercase()),
            title: format!("endoflife.date: {} release and support status", slug),
            url: format!("https://endoflife.date/{}", slug),
            integrity_hash: format!("{:x}", Sha256::digest(content.as_bytes())),
            content,
            is_sanitized: true,
        };
        self.cache.write().await.insert(slug.to_string(), (chunk.clone(), Instant::now()));
        Some(chunk)
    }
}

fn scalar(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Newest `MAX_CYCLES` release cycles, as plain text. The API returns newest first.
pub fn format_cycles(slug: &str, cycles: &[Value]) -> Option<String> {
    if cycles.is_empty() {
        return None;
    }
    let mut s = format!("Product: {}\nSource: endoflife.date (live API)\n", slug);
    for c in cycles.iter().take(MAX_CYCLES) {
        let cycle = scalar(c.get("cycle")).unwrap_or_else(|| "?".into());
        let mut line = format!("Release line {}:", cycle);
        if let Some(l) = scalar(c.get("latest")) {
            line.push_str(&format!(" latest version {}", l));
        }
        if let Some(d) = scalar(c.get("latestReleaseDate")).or_else(|| scalar(c.get("releaseDate"))) {
            line.push_str(&format!(" (released {})", d));
        }
        if let Some(e) = scalar(c.get("eol")) {
            line.push_str(&format!("; end of life: {}", e));
        }
        if let Some(l) = scalar(c.get("lts")) {
            line.push_str(&format!("; LTS: {}", l));
        }
        s.push_str(&line);
        s.push('\n');
    }
    Some(s)
}
