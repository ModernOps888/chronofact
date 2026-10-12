use super::extractor::{ContentExtractor, SourceChunk};
use crate::security::{ContentSanitizer, SecurityValidator};
use scraper::{Html, Selector};
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

pub struct SearchEngine {
    client: reqwest::Client,
    sanitizer: Arc<ContentSanitizer>,
    package_client: Arc<super::package_registry::PackageRegistryClient>,
    eol_client: Arc<super::eol::EolClient>,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Unix seconds until which the DuckDuckGo scraper is considered blocked (anti-bot challenge).
static DDG_BLOCKED_UNTIL: AtomicU64 = AtomicU64::new(0);
const DDG_BLOCK_SECS: u64 = 900;
const DDG_ERROR_BACKOFF_SECS: u64 = 120;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// True if a search-engine response is an anti-bot wall rather than results
/// (DuckDuckGo answers 202 + `anomaly` form, 403, or 429).
pub fn is_block_response(status: u16, body: &str) -> bool {
    matches!(status, 202 | 403 | 429) || body.contains("anomaly.js") || body.contains("anomaly-modal")
}

/// Cheap relevance gate: engines that serve a degraded index to bots return unrelated pages
/// (observed: "latest axum crate" -> UK news). A result must share a content word with the query.
pub fn result_is_relevant(query: &str, title: &str, snippet: &str, url: &str) -> bool {
    const GENERIC: &[&str] = &[
        "the", "and", "for", "what", "which", "latest", "newest", "new", "version", "release", "released",
        "stable", "current", "how", "when", "does", "with", "from", "that", "this", "are", "was", "has",
        "have", "about", "into", "using", "use",
    ];
    let q = query.to_lowercase();
    let words: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3 && !GENERIC.contains(w))
        .collect();
    if words.is_empty() {
        return true;
    }
    let hay = format!("{} {} {}", title, snippet, url).to_lowercase();
    let hits = words.iter().filter(|w| hay.contains(**w)).count();
    // one content word suffices for short queries; longer queries need two
    hits >= if words.len() >= 3 { 2 } else { 1 }
}

impl SearchEngine {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) ChronoFact/0.1.0")
            .build()
            .unwrap_or_default();

        Self {
            client,
            sanitizer: Arc::new(ContentSanitizer::new()),
            package_client: Arc::new(super::package_registry::PackageRegistryClient::new()),
            eol_client: Arc::new(super::eol::EolClient::new()),
        }
    }

    pub async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SourceChunk>, String> {
        let mut results = Vec::new();

        // 0. Live keyless API evidence: package registries (curated + explicitly named packages)
        //    and endoflife.date release/support status. This is third-party content: it goes through
        //    the same SSRF check and sanitizer as web results, and the integrity hash is of the
        //    original text.
        let mut api_sources = self.package_client.ground_query_packages(query).await;
        api_sources.extend(self.eol_client.ground_query(query).await);
        for mut s in api_sources {
            if results.len() >= max_results {
                break;
            }
            if SecurityValidator::validate_outbound_url(&s.url).is_err() {
                continue;
            }
            let sanitized = self.sanitizer.sanitize_external_evidence(&s.content, &s.url);
            s.content = sanitized.safe_text;
            s.integrity_hash = sanitized.original_hash;
            s.is_sanitized = true;
            results.push(s);
        }

        if results.len() >= max_results {
            return Ok(results);
        }

        let remaining = max_results - results.len();

        // 1. Check if user configured Tavily API
        if let Ok(tavily_key) = env::var("TAVILY_API_KEY") {
            if !tavily_key.trim().is_empty() {
                if let Ok(web_results) = self.search_tavily(&tavily_key, query, remaining).await {
                    results.extend(web_results);
                    if results.len() >= max_results {
                        return Ok(results);
                    }
                }
            }
        }

        // 2. Default: DuckDuckGo HTML scraper
        if results.len() < max_results {
            if let Ok(ddg_results) = self.search_duckduckgo(query, max_results - results.len()).await {
                results.extend(ddg_results);
            }
        }

        Ok(results)
    }

    async fn search_duckduckgo(&self, query: &str, max_results: usize) -> Result<Vec<SourceChunk>, String> {
        if unix_now() < DDG_BLOCKED_UNTIL.load(Ordering::Relaxed) {
            return Ok(Vec::new());
        }
        let url = "https://html.duckduckgo.com/html/";
        let params = [("q", query)];

        let response = match self.client.post(url).timeout(Duration::from_secs(4)).form(&params).send().await {
            Ok(resp) if resp.status().is_success() && resp.status().as_u16() != 202 => resp,
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                if is_block_response(status, &body) {
                    DDG_BLOCKED_UNTIL.store(unix_now() + DDG_BLOCK_SECS, Ordering::Relaxed);
                    warn!("DuckDuckGo anti-bot wall (HTTP {}); web scraper paused for {}s", status, DDG_BLOCK_SECS);
                } else {
                    warn!("DuckDuckGo search returned non-success HTTP status: {}", status);
                }
                return Ok(Vec::new());
            }
            Err(e) => {
                // Timeouts/connection resets are how some networks "block" scrapers: back off briefly.
                DDG_BLOCKED_UNTIL.store(unix_now() + DDG_ERROR_BACKOFF_SECS, Ordering::Relaxed);
                warn!("DuckDuckGo search request failed: {}; web scraper paused for {}s", e, DDG_ERROR_BACKOFF_SECS);
                return Ok(Vec::new());
            }
        };

        let html_text = response.text().await.unwrap_or_default();
        if is_block_response(200, &html_text) {
            DDG_BLOCKED_UNTIL.store(unix_now() + DDG_BLOCK_SECS, Ordering::Relaxed);
            warn!("DuckDuckGo served an anti-bot challenge; web scraper paused for {}s", DDG_BLOCK_SECS);
            return Ok(Vec::new());
        }
        let document = Html::parse_document(&html_text);

        let result_selector = Selector::parse(".result").map_err(|e| format!("{:?}", e))?;
        let title_selector = Selector::parse(".result__title a").map_err(|e| format!("{:?}", e))?;
        let snippet_selector = Selector::parse(".result__snippet").map_err(|e| format!("{:?}", e))?;

        let mut chunks = Vec::new();
        let mut idx = 1;

        for element in document.select(&result_selector) {
            if chunks.len() >= max_results {
                break;
            }

            let title = element.select(&title_selector).next()
                .map(|e| e.text().collect::<Vec<_>>().join(" "))
                .unwrap_or_else(|| "Search Result".to_string());

            let raw_url = element.select(&title_selector).next()
                .and_then(|e| e.value().attr("href"))
                .unwrap_or("");

            // DDG href may be udn-wrapped (/l/?kh=-1&uddg=https%3A%2F%2F...)
            let resolved_url = if raw_url.contains("uddg=") {
                raw_url.split("uddg=")
                    .nth(1)
                    .and_then(|u| u.split('&').next())
                    .map(|u| urlencoding::decode(u).unwrap_or_default().to_string())
                    .unwrap_or_else(|| raw_url.to_string())
            } else {
                raw_url.to_string()
            };

            let snippet = element.select(&snippet_selector).next()
                .map(|e| e.text().collect::<Vec<_>>().join(" "))
                .unwrap_or_default();

            if snippet.trim().is_empty() || resolved_url.trim().is_empty() {
                continue;
            }

            // Degraded-index guard: drop results unrelated to the query
            if !result_is_relevant(query, &title, &snippet, &resolved_url) {
                continue;
            }

            // Security: SSRF validation on target URL
            if SecurityValidator::validate_outbound_url(&resolved_url).is_err() {
                continue;
            }

            // Security: Sanitize untrusted web evidence against indirect prompt injection
            let sanitized = self.sanitizer.sanitize_external_evidence(&snippet, &resolved_url);

            chunks.push(SourceChunk {
                id: format!("SRC-{}", idx),
                title: ContentExtractor::clean_text(&title),
                url: resolved_url,
                content: sanitized.safe_text,
                integrity_hash: sanitized.original_hash,
                is_sanitized: true,
            });

            idx += 1;
        }

        Ok(chunks)
    }

    async fn search_tavily(&self, api_key: &str, query: &str, max_results: usize) -> Result<Vec<SourceChunk>, String> {
        let payload = serde_json::json!({
            "api_key": api_key,
            "query": query,
            "max_results": max_results,
            "search_depth": "basic",
        });

        let resp = self.client.post("https://api.tavily.com/search")
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let mut chunks = Vec::new();

        if let Some(results) = data.get("results").and_then(|r| r.as_array()) {
            for (idx, item) in results.iter().enumerate() {
                let title = item.get("title").and_then(|t| t.as_str()).unwrap_or("Search Result");
                let url = item.get("url").and_then(|u| u.as_str()).unwrap_or("");
                let content = item.get("content").and_then(|c| c.as_str()).unwrap_or("");

                if SecurityValidator::validate_outbound_url(url).is_err() {
                    continue;
                }

                let sanitized = self.sanitizer.sanitize_external_evidence(content, url);

                chunks.push(SourceChunk {
                    id: format!("SRC-{}", idx + 1),
                    title: title.to_string(),
                    url: url.to_string(),
                    content: sanitized.safe_text,
                    integrity_hash: sanitized.original_hash,
                    is_sanitized: true,
                });
            }
        }

        Ok(chunks)
    }
}
