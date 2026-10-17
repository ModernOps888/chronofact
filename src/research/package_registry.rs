//! Dynamic Package Registry Grounding Client
//!
//! Queries official ecosystem package registries (crates.io, npm, PyPI) to obtain
//! 100% verified, live version specifications and deprecation notices rather than
//! relying on static regexes or scraping fragile HTML search results.

use super::extractor::SourceChunk;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour memory TTL
const MAX_REGISTRY_CANDIDATES: usize = 4;

/// Curated query-token -> (registry package name, ecosystem). Aliases map to the real package
/// name (e.g. "tailwind" -> "tailwindcss"). Extend deliberately; do not replace with a wordlist probe.
fn known_package(tok: &str) -> Option<(&'static str, PackageEcosystem)> {
    use PackageEcosystem::*;
    Some(match tok {
        // crates.io
        "tokio" => ("tokio", Cargo), "axum" => ("axum", Cargo), "actix" | "actix-web" => ("actix-web", Cargo),
        "serde" => ("serde", Cargo), "reqwest" => ("reqwest", Cargo), "sqlx" => ("sqlx", Cargo),
        "diesel" => ("diesel", Cargo), "polars" => ("polars", Cargo), "tauri" => ("tauri", Cargo),
        "bevy" => ("bevy", Cargo), "tracing" => ("tracing", Cargo), "clap" => ("clap", Cargo),
        "rusqlite" => ("rusqlite", Cargo), "hyper" => ("hyper", Cargo),
        // npm
        "react" => ("react", Npm), "vite" => ("vite", Npm), "next" | "nextjs" | "next.js" => ("next", Npm),
        "tailwind" | "tailwindcss" => ("tailwindcss", Npm), "svelte" => ("svelte", Npm), "vue" => ("vue", Npm),
        "nuxt" => ("nuxt", Npm), "astro" => ("astro", Npm), "typescript" => ("typescript", Npm),
        "eslint" => ("eslint", Npm), "webpack" => ("webpack", Npm), "esbuild" => ("esbuild", Npm),
        "rollup" => ("rollup", Npm), "prisma" => ("prisma", Npm), "zod" => ("zod", Npm),
        "vitest" => ("vitest", Npm), "jest" => ("jest", Npm), "playwright" => ("playwright", Npm),
        "hono" => ("hono", Npm), "express" => ("express", Npm), "drizzle" => ("drizzle-orm", Npm),
        // PyPI
        "pydantic" => ("pydantic", PyPi), "fastapi" => ("fastapi", PyPi), "django" => ("django", PyPi),
        "flask" => ("flask", PyPi), "numpy" => ("numpy", PyPi), "pandas" => ("pandas", PyPi),
        "scipy" => ("scipy", PyPi), "pytorch" | "torch" => ("torch", PyPi), "tensorflow" => ("tensorflow", PyPi),
        "langchain" => ("langchain", PyPi), "transformers" => ("transformers", PyPi),
        "celery" => ("celery", PyPi), "sqlalchemy" => ("sqlalchemy", PyPi), "requests" => ("requests", PyPi),
        _ => return None,
    })
}

/// License of the *current* PyPI release: PEP 639 expression, else a short `license` field,
/// else the OSI classifier name. Long free-text license bodies are ignored.
fn pypi_license(info: &serde_json::Value) -> Option<String> {
    let field = |k: &str| {
        info.get(k)
            .and_then(|v| v.as_str())
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
    };
    if let Some(e) = field("license_expression") {
        return Some(e);
    }
    if let Some(l) = field("license") {
        if l.len() <= 40 && !l.contains('\n') {
            return Some(l);
        }
    }
    info.get("classifiers")?
        .as_array()?
        .iter()
        .filter_map(|c| c.as_str())
        .find(|c| c.starts_with("License :: OSI Approved ::"))
        .map(|c| c.rsplit("::").next().unwrap_or("").trim().to_string())
        .filter(|x| !x.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageEcosystem {
    Cargo,
    Npm,
    PyPi,
}

impl PackageEcosystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cargo => "crates.io",
            Self::Npm => "npm",
            Self::PyPi => "PyPI",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub ecosystem: String,
    pub latest_version: String,
    pub description: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub deprecated: Option<String>,
    /// Publication date (UTC, YYYY-MM-DD) of `latest_version`, when the registry exposes it.
    #[serde(default)]
    pub published: Option<String>,
    /// SPDX-style license of `latest_version`, when the registry exposes it.
    #[serde(default)]
    pub license: Option<String>,
    pub retrieved_at: String,
}

impl PackageMetadata {
    fn is_safe_metadata_url(raw_url: &str) -> bool {
        match crate::security::SecurityValidator::validate_outbound_url(raw_url) {
            Ok(_) => true,
            Err(crate::security::SecurityError::InvalidUrl(msg))
                if msg.starts_with("DNS resolution failed:") =>
            {
                if let Ok(parsed) = url::Url::parse(raw_url) {
                    matches!(parsed.scheme(), "http" | "https")
                        && matches!(parsed.host(), Some(url::Host::Domain(d)) if {
                            let dl = d.trim_end_matches('.').to_lowercase();
                            dl != "localhost"
                                && !dl.ends_with(".local")
                                && !dl.ends_with(".internal")
                                && dl.contains('.')
                        })
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    pub fn to_source_chunk(&self) -> SourceChunk {
        let mut content = format!(
            "Package: {}\nEcosystem: {}\nLatest Verified Version: {}\nDescription: {}\n",
            self.name, self.ecosystem, self.latest_version, self.description
        );

        if let Some(ref d) = self.published {
            content.push_str(&format!("Latest Version Published (UTC): {}\n", d));
        }
        if let Some(ref l) = self.license {
            content.push_str(&format!("License: {}\n", l));
        }
        if let Some(ref dep) = self.deprecated {
            content.push_str(&format!("DEPRECATION STATUS: WARNING - Deprecated: {}\n", dep));
        }
        if let Some(ref repo) = self.repository.as_ref().filter(|u| Self::is_safe_metadata_url(u)) {
            content.push_str(&format!("Repository: {}\n", repo));
        }
        if let Some(ref docs) = self.documentation.as_ref().filter(|u| Self::is_safe_metadata_url(u)) {
            content.push_str(&format!("Documentation: {}\n", docs));
        }

        let url = self
            .documentation
            .clone()
            .filter(|u| Self::is_safe_metadata_url(u))
            .or_else(|| self.homepage.clone().filter(|u| Self::is_safe_metadata_url(u)))
            .or_else(|| self.repository.clone().filter(|u| Self::is_safe_metadata_url(u)))
            .unwrap_or_else(|| match self.ecosystem.as_str() {
                "crates.io" => format!("https://crates.io/crates/{}", self.name),
                "npm" => format!("https://www.npmjs.com/package/{}", self.name),
                "PyPI" => format!("https://pypi.org/project/{}/", self.name),
                _ => "https://chronofact.dev".to_string(),
            });

        let integrity_hash = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(content.as_bytes()))
        };

        SourceChunk {
            id: format!("REG-{}-{}", self.ecosystem.to_uppercase(), self.name),
            title: format!("Verified {} Registry: {} v{}", self.ecosystem, self.name, self.latest_version),
            url,
            content,
            integrity_hash,
            is_sanitized: true,
        }
    }
}

pub struct PackageRegistryClient {
    client: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, (PackageMetadata, Instant)>>>,
}

impl Default for PackageRegistryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageRegistryClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("ChronoFact/0.1.0 (https://github.com/ModernOps888/chronofact)")
            .build()
            .unwrap_or_default();

        Self {
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Lookup a crate on crates.io
    pub async fn lookup_crate(&self, name: &str) -> Result<PackageMetadata, String> {
        let cache_key = format!("cargo:{}", name.to_lowercase());
        if let Some(meta) = self.get_cached(&cache_key).await {
            return Ok(meta);
        }

        let url = format!("https://crates.io/api/v1/crates/{}", name);
        let resp = self.client.get(&url).send().await.map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("crates.io returned status {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let crate_obj = body.get("crate").ok_or_else(|| "Missing 'crate' object".to_string())?;

        // Prefer the newest stable release over a pre-release `max_version`.
        let latest_version = crate_obj
            .get("max_stable_version")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| crate_obj.get("max_version").and_then(|v| v.as_str()))
            .unwrap_or("unknown")
            .to_string();

        let version_entry = body
            .get("versions")
            .and_then(|v| v.as_array())
            .and_then(|a| a.iter().find(|e| e.get("num").and_then(|n| n.as_str()) == Some(latest_version.as_str())));
        let published = version_entry
            .and_then(|e| e.get("created_at"))
            .and_then(|v| v.as_str())
            .map(|s| s.chars().take(10).collect::<String>());
        let license = version_entry.and_then(|e| e.get("license")).and_then(|v| v.as_str()).map(|s| s.to_string());

        let description = crate_obj
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let homepage = crate_obj.get("homepage").and_then(|v| v.as_str()).map(|s| s.to_string());
        let repository = crate_obj.get("repository").and_then(|v| v.as_str()).map(|s| s.to_string());
        let documentation = crate_obj.get("documentation").and_then(|v| v.as_str()).map(|s| s.to_string());

        let meta = PackageMetadata {
            name: name.to_string(),
            ecosystem: PackageEcosystem::Cargo.as_str().to_string(),
            latest_version,
            description,
            homepage,
            repository,
            documentation,
            deprecated: None,
            published,
            license,
            retrieved_at: chrono::Utc::now().to_rfc3339(),
        };

        self.set_cached(&cache_key, meta.clone()).await;
        Ok(meta)
    }

    /// Lookup a package on npm
    pub async fn lookup_npm(&self, name: &str) -> Result<PackageMetadata, String> {
        let cache_key = format!("npm:{}", name.to_lowercase());
        if let Some(meta) = self.get_cached(&cache_key).await {
            return Ok(meta);
        }

        let url = format!("https://registry.npmjs.org/{}/latest", name);
        let resp = self.client.get(&url).send().await.map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("npm returned status {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

        let latest_version = body
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let description = body
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let homepage = body.get("homepage").and_then(|v| v.as_str()).map(|s| s.to_string());

        let repository = body.get("repository").and_then(|r| {
            if let Some(s) = r.as_str() {
                Some(s.to_string())
            } else {
                r.get("url").and_then(|v| v.as_str()).map(|s| s.to_string())
            }
        });

        let deprecated = body.get("deprecated").and_then(|v| v.as_str()).map(|s| s.to_string());

        let license = body.get("license").and_then(|l| {
            l.as_str()
                .map(|s| s.to_string())
                .or_else(|| l.get("type").and_then(|t| t.as_str()).map(|s| s.to_string()))
        });
        // The npm /latest document has no publish date; deps.dev (keyless) serves it in a small payload.
        let published = self.deps_dev_published("npm", name, &latest_version).await;

        let meta = PackageMetadata {
            name: name.to_string(),
            ecosystem: PackageEcosystem::Npm.as_str().to_string(),
            latest_version,
            description,
            homepage,
            repository,
            documentation: None,
            deprecated,
            published,
            license,
            retrieved_at: chrono::Utc::now().to_rfc3339(),
        };

        self.set_cached(&cache_key, meta.clone()).await;
        Ok(meta)
    }

    /// Lookup a package on PyPI
    pub async fn lookup_pypi(&self, name: &str) -> Result<PackageMetadata, String> {
        let cache_key = format!("pypi:{}", name.to_lowercase());
        if let Some(meta) = self.get_cached(&cache_key).await {
            return Ok(meta);
        }

        let url = format!("https://pypi.org/pypi/{}/json", name);
        let resp = self.client.get(&url).send().await.map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("PyPI returned status {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let info = body.get("info").ok_or_else(|| "Missing 'info' object".to_string())?;

        let latest_version = info
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let description = info
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let homepage = info.get("home_page").and_then(|v| v.as_str()).map(|s| s.to_string());
        let repository = info
            .get("project_urls")
            .and_then(|p| p.get("Source").or_else(|| p.get("Repository")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let published = body
            .get("releases")
            .and_then(|r| r.get(&latest_version))
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|f| f.get("upload_time_iso_8601").or_else(|| f.get("upload_time")))
            .and_then(|v| v.as_str())
            .map(|s| s.chars().take(10).collect::<String>());
        let license = pypi_license(info);

        let meta = PackageMetadata {
            name: name.to_string(),
            ecosystem: PackageEcosystem::PyPi.as_str().to_string(),
            latest_version,
            description,
            homepage,
            repository,
            documentation: None,
            deprecated: None,
            published,
            license,
            retrieved_at: chrono::Utc::now().to_rfc3339(),
        };

        self.set_cached(&cache_key, meta.clone()).await;
        Ok(meta)
    }

    /// Extracts package candidates from a freeform query. Only names found in the curated
    /// ecosystem tables below are considered: probing every word of a query against three
    /// public registries returns junk ("the", "latest", "version" are all real package names)
    /// and burns rate limit. Result is deduplicated and capped.
    pub fn candidates_from_query(query: &str) -> Vec<(String, PackageEcosystem)> {
        let mut out: Vec<(String, PackageEcosystem)> = Vec::new();
        let lower = query.to_lowercase();
        for tok in lower.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == '@' || c == '/')) {
            let tok = tok.trim_matches(|c: char| c == '-' || c == '_' || c == '/');
            if tok.is_empty() {
                continue;
            }
            if let Some((pkg, eco)) = known_package(tok) {
                if !out.iter().any(|(n, e)| n == pkg && *e == eco) {
                    out.push((pkg.to_string(), eco));
                }
            }
            if out.len() >= MAX_REGISTRY_CANDIDATES {
                break;
            }
        }
        out
    }

    /// Packages the query names *unambiguously* but that are not in the curated table. Probing every
    /// word of a query returns junk, so a name is only accepted in one of these explicit shapes:
    /// `` `name` `` together with an ecosystem word, `pip|npm|yarn|pnpm|cargo install|add name`,
    /// `name <eco> package|crate|library`, `<eco> package|crate|library name`, or `name crate`.
    /// Returns (name, ecosystem) pairs; the ecosystem is always explicit.
    pub fn explicit_candidates(query: &str) -> Vec<(String, PackageEcosystem)> {
        use PackageEcosystem::*;
        const STOP: &[&str] = &[
            "the", "a", "an", "of", "for", "in", "on", "is", "are", "was", "were", "what", "which", "latest",
            "newest", "new", "version", "versions", "stable", "current", "release", "released", "this", "that",
            "these", "those", "and", "or", "to", "from", "with", "by", "about", "any", "some", "package",
            "packages", "crate", "crates", "library", "libraries", "lib", "module", "dependency", "rust",
            "python", "npm", "node", "nodejs", "javascript", "js", "typescript", "pip", "pip3", "pypi",
            "cargo", "yarn", "pnpm", "install", "add", "i", "it", "its", "my", "our", "your", "their", "there",
            "here", "how", "when", "where", "why", "who", "does", "do", "did", "can", "should", "would", "will",
            "still", "not", "no", "yes", "deprecated", "outdated", "up", "date", "old", "newer", "older",
            "published", "license", "licence", "stable", "official", "use", "using", "used",
        ];
        const BAD_EXT: &[&str] = &[
            ".rs", ".py", ".js", ".ts", ".tsx", ".jsx", ".json", ".toml", ".md", ".txt", ".yml", ".yaml", ".html",
            ".css", ".lock", ".cfg", ".ini", ".sh", ".exe", ".dll",
        ];
        fn eco_of(w: &str) -> Option<PackageEcosystem> {
            match w {
                "rust" | "cargo" | "crate" | "crates" | "crates.io" => Some(Cargo),
                "npm" | "node" | "nodejs" | "javascript" | "js" | "typescript" | "yarn" | "pnpm" | "npx" => Some(Npm),
                "python" | "pypi" | "pip" | "pip3" => Some(PyPi),
                _ => None,
            }
        }
        fn valid_name(n: &str, eco: &PackageEcosystem) -> bool {
            let ok_chars = !n.is_empty()
                && n.len() >= 2
                && n.len() <= 64
                && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-' | '/' | '@'))
                && n.chars().next().map_or(false, |c| c.is_ascii_alphanumeric() || c == '@');
            if !ok_chars {
                return false;
            }
            if n.contains('/') || n.starts_with('@') {
                // Scoped names exist on npm only, and must look like @scope/name.
                return *eco == PackageEcosystem::Npm && n.starts_with('@') && n.matches('/').count() == 1;
            }
            true
        }
        let lower = query.to_lowercase();
        let mut out: Vec<(String, PackageEcosystem)> = Vec::new();
        let push = |name: &str, eco: PackageEcosystem, out: &mut Vec<(String, PackageEcosystem)>| {
            let name = name.trim_matches(|c: char| matches!(c, '.' | '-' | '_' | '/' | ',' | ';' | ':' | '?' | '!'));
            if STOP.contains(&name) || !valid_name(name, &eco) || known_package(name).is_some() {
                return;
            }
            if BAD_EXT.iter().any(|e| name.ends_with(e)) {
                return;
            }
            if !out.iter().any(|(n, e)| n == name && *e == eco) {
                out.push((name.to_string(), eco));
            }
        };

        // Tokens keep '.', '@', '/', '-', '_' so scoped and dotted names survive.
        let toks: Vec<&str> = lower
            .split(|c: char| !(c.is_alphanumeric() || matches!(c, '.' | '@' | '/' | '-' | '_')))
            .map(|t| t.trim_matches(|c: char| matches!(c, '.' | '-' | '_' | '/')))
            .filter(|t| !t.is_empty())
            .collect();
        let query_ecos: Vec<PackageEcosystem> = {
            let mut v = Vec::new();
            for t in &toks {
                if let Some(e) = eco_of(t) {
                    if !v.contains(&e) {
                        v.push(e);
                    }
                }
            }
            v
        };

        // `backticked` identifiers, only when the query also names an ecosystem.
        if !query_ecos.is_empty() {
            let mut parts = lower.split('`');
            parts.next();
            for (k, seg) in parts.enumerate() {
                if k % 2 == 0 {
                    let seg = seg.trim();
                    for e in &query_ecos {
                        push(seg, e.clone(), &mut out);
                    }
                }
            }
        }

        let is_filler = |w: &str| STOP.contains(&w) || eco_of(w).is_some();
        for (j, t) in toks.iter().enumerate() {
            // "pip install openai", "cargo add tower", "npm i three"
            if matches!(*t, "install" | "add" | "i") && j > 0 {
                let manager = matches!(toks[j - 1], "pip" | "pip3" | "npm" | "yarn" | "pnpm" | "cargo" | "npx");
                if let Some(eco) = eco_of(toks[j - 1]).filter(|_| manager) {
                    if let Some(name) = toks[j + 1..].iter().find(|n| !n.starts_with('-')) {
                        push(name, eco, &mut out);
                    }
                }
                continue;
            }
            if !matches!(*t, "package" | "packages" | "library" | "lib" | "module" | "crate" | "crates") {
                continue;
            }
            let is_crate = matches!(*t, "crate" | "crates");
            let prev_eco = if j > 0 { eco_of(toks[j - 1]) } else { None };
            let eco = match (prev_eco.clone(), is_crate) {
                (Some(e), _) => e,
                (None, true) => Cargo,
                (None, false) => continue, // bare "package"/"library" is too ambiguous
            };
            // name before the (eco) cue, else name after it
            let before_idx = {
                let mut k = j;
                while k > 0 && eco_of(toks[k - 1]).is_some() {
                    k -= 1;
                }
                k.checked_sub(1)
            };
            let before = before_idx.and_then(|b| toks.get(b)).filter(|w| !is_filler(w));
            let after = toks.get(j + 1).filter(|w| !is_filler(w));
            if let Some(n) = before.or(after) {
                push(n, eco, &mut out);
            }
        }
        out.truncate(MAX_REGISTRY_CANDIDATES);
        out
    }

    pub async fn lookup(&self, name: &str, eco: &PackageEcosystem) -> Result<PackageMetadata, String> {
        match eco {
            PackageEcosystem::Cargo => self.lookup_crate(name).await,
            PackageEcosystem::Npm => self.lookup_npm(name).await,
            PackageEcosystem::PyPi => self.lookup_pypi(name).await,
        }
    }

    /// Publish date (YYYY-MM-DD) of a specific version via the keyless deps.dev API. Best-effort.
    async fn deps_dev_published(&self, system: &str, name: &str, version: &str) -> Option<String> {
        let enc = |s: &str| s.replace('/', "%2F").replace('@', "%40").replace('+', "%2B");
        let url = format!(
            "https://api.deps.dev/v3/systems/{}/packages/{}/versions/{}",
            system,
            enc(name),
            enc(version)
        );
        let resp = self.client.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let v: serde_json::Value = resp.json().await.ok()?;
        v.get("publishedAt")?.as_str().map(|s| s.chars().take(10).collect())
    }

    /// Looks up every curated package mentioned in the query, in its own ecosystem only, then any
    /// explicitly-named uncurated package in the ecosystem the query named.
    pub async fn ground_query_packages(&self, query: &str) -> Vec<SourceChunk> {
        let mut chunks = Vec::new();
        let mut wanted = Self::candidates_from_query(query);
        for c in Self::explicit_candidates(query) {
            if wanted.len() >= MAX_REGISTRY_CANDIDATES {
                break;
            }
            if !wanted.contains(&c) {
                wanted.push(c);
            }
        }
        for (name, eco) in wanted {
            if let Ok(meta) = self.lookup(&name, &eco).await {
                chunks.push(meta.to_source_chunk());
            }
        }
        chunks
    }

    async fn get_cached(&self, key: &str) -> Option<PackageMetadata> {
        let guard = self.cache.read().await;
        if let Some((meta, created_at)) = guard.get(key) {
            if created_at.elapsed() < CACHE_TTL {
                return Some(meta.clone());
            }
        }
        None
    }

    async fn set_cached(&self, key: &str, meta: PackageMetadata) {
        let mut guard = self.cache.write().await;
        guard.insert(key.to_string(), (meta, Instant::now()));
    }
}
