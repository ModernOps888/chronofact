use super::extractor::{ContentExtractor, SourceChunk};
use crate::security::{ContentSanitizer, SecurityValidator};
use scraper::{Html, Selector};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

pub struct SearchEngine {
    client: reqwest::Client,
    sanitizer: Arc<ContentSanitizer>,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
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
        }
    }

    pub async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SourceChunk>, String> {
        // 1. Check if user configured Tavily API
        if let Ok(tavily_key) = env::var("TAVILY_API_KEY") {
            if !tavily_key.trim().is_empty() {
                if let Ok(results) = self.search_tavily(&tavily_key, query, max_results).await {
                    if !results.is_empty() {
                        return Ok(results);
                    }
                }
            }
        }

        // 2. Default: DuckDuckGo HTML scraper
        self.search_duckduckgo(query, max_results).await
    }

    async fn search_duckduckgo(&self, query: &str, max_results: usize) -> Result<Vec<SourceChunk>, String> {
        let url = "https://html.duckduckgo.com/html/";
        let params = [("q", query)];

        let response = match self.client.post(url).form(&params).send().await {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                warn!("DuckDuckGo search returned non-success HTTP status: {}", resp.status());
                return Ok(Vec::new());
            }
            Err(e) => {
                warn!("DuckDuckGo search request failed: {}", e);
                return Ok(Vec::new());
            }
        };

        let html_text = response.text().await.unwrap_or_default();
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
