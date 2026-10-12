//! Keyless grounding: explicit package extraction, endoflife.date shaping, scraper circuit-breaker
//! helpers, relevance gate and compact temporal anchor. All offline (no network).
use chrono::NaiveDate;
use chronofact::research::eol::{format_cycles, has_release_intent, slug_candidates};
use chronofact::research::searcher::{is_block_response, result_is_relevant};
use chronofact::research::{PackageEcosystem, PackageMetadata, PackageRegistryClient};
use chronofact::temporal::{HorizonCalculator, ModelRegistry, TemporalScanner};

fn names(q: &str) -> Vec<(String, PackageEcosystem)> {
    PackageRegistryClient::explicit_candidates(q)
}

fn has(q: &str, n: &str, e: PackageEcosystem) -> bool {
    names(q).iter().any(|(a, b)| a == n && *b == e)
}

#[test]
fn explicit_candidates_accepts_only_unambiguous_shapes() {
    use PackageEcosystem::*;
    assert!(has("What is the latest stable version of the PyPI package `openai`?", "openai", PyPi));
    assert!(has("latest version of the regex rust crate", "regex", Cargo));
    assert!(has("which version of the rust crate anyhow is current", "anyhow", Cargo));
    assert!(has("npm package three latest release", "three", Npm));
    assert!(has("pip install anthropic gives which version", "anthropic", PyPi));
    assert!(has("cargo add tower, what version?", "tower", Cargo));
    assert!(has("latest version of the npm package `@types/node`", "@types/node", Npm));
    assert!(has("when was `candle-core` last released on crates.io", "candle-core", Cargo));
    // stacked ecosystem words before the cue (found by the live A/B)
    assert!(has("license of the openai python pypi package", "openai", PyPi));
    assert!(has("latest version of the anthropic python pypi package", "anthropic", PyPi));
}

#[test]
fn explicit_candidates_never_probes_ordinary_words() {
    // Prose that merely mentions "package", "library", "install" or "add" must not be probed.
    for q in [
        "what is the latest version of the thing",
        "the package released yesterday broke my build",
        "add numbers in python",
        "python add numbers to a list",
        "explain the rust standard library",
        "install the latest updates",
        "I use `foo` in my code",          // backticks without an ecosystem word
        "open `src/main.rs` in the rust project",
        "check `config.json` for the node app",
        "use the library for parsing",     // bare 'library' is ambiguous
    ] {
        let got = names(q);
        assert!(got.is_empty(), "unexpected probe {:?} for {:?}", got, q);
    }
}

#[test]
fn explicit_candidates_skip_curated_and_reject_unsafe_names() {
    // curated names are handled by the curated path
    assert!(names("the tokio rust crate").is_empty());
    // path traversal / URL tricks are rejected
    assert!(names("npm package `../../etc/passwd`").is_empty());
    assert!(names("rust crate `a/b`").is_empty());
    // scoped names are npm-only
    assert!(names("rust crate `@x/y`").is_empty());
    assert!(has("npm package `@x/y`", "@x/y", PackageEcosystem::Npm));
}

#[test]
fn registry_chunk_carries_date_and_license() {
    let m = PackageMetadata {
        name: "axum".into(),
        ecosystem: "crates.io".into(),
        latest_version: "0.8.9".into(),
        description: "web framework".into(),
        homepage: None,
        repository: Some("https://github.com/tokio-rs/axum".into()),
        documentation: None,
        deprecated: None,
        published: Some("2026-04-14".into()),
        license: Some("MIT".into()),
        retrieved_at: "now".into(),
    };
    let c = m.to_source_chunk().content;
    assert!(c.contains("Latest Version Published (UTC): 2026-04-14"));
    assert!(c.contains("License: MIT"));
    assert!(c.contains("Latest Verified Version: 0.8.9"));
}

#[test]
fn eol_intent_and_slugs() {
    assert!(has_release_intent("what is the latest python version"));
    assert!(!has_release_intent("explain python decorators"));
    let s = slug_candidates("is node 18 still supported and what is the latest golang release");
    assert!(s.contains(&"nodejs".to_string()), "{:?}", s);
    assert!(s.contains(&"go".to_string()), "{:?}", s);
    // "go" as an ordinary word is not a product
    assert!(!slug_candidates("go ahead and show the latest results").contains(&"go".to_string()));
    // a short slug followed by a version number is
    assert!(slug_candidates("latest go 1.25 patch release").contains(&"go".to_string()));
    // "rust crate" / "python package" qualify a package; they are not questions about the language
    assert!(!slug_candidates("latest version of the rust crate chrono").contains(&"rust".to_string()));
    assert!(!slug_candidates("latest python package openai").contains(&"python".to_string()));
    assert!(slug_candidates("latest rust release").contains(&"rust".to_string()));
}

#[test]
fn eol_chunk_formatting_handles_mixed_types() {
    let v: serde_json::Value = serde_json::json!([
        {"cycle": "3.15", "latest": "3.15.0", "latestReleaseDate": "2026-10-09", "eol": "2031-10-31", "lts": false},
        {"cycle": 24, "latest": "24.1.0", "releaseDate": "2026-05-01", "eol": false, "lts": "2026-10-28"},
    ]);
    let s = format_cycles("python", v.as_array().unwrap()).unwrap();
    assert!(s.contains("Release line 3.15: latest version 3.15.0 (released 2026-10-09); end of life: 2031-10-31; LTS: false"));
    assert!(s.contains("Release line 24: latest version 24.1.0 (released 2026-05-01); end of life: false; LTS: 2026-10-28"));
    assert!(format_cycles("x", &[]).is_none());
}

#[test]
fn scraper_block_detection_and_relevance_gate() {
    assert!(is_block_response(202, ""));
    assert!(is_block_response(403, ""));
    assert!(is_block_response(429, ""));
    assert!(is_block_response(200, "<form action=\"//duckduckgo.com/anomaly.js?x\">"));
    assert!(!is_block_response(200, "<div class=\"result\">axum</div>"));

    // observed degraded-index garbage must be rejected, real hits accepted
    let q = "latest version of axum rust crate";
    assert!(!result_is_relevant(q, "UK | Latest News & Updates - BBC", "Get all the latest news about the UK", "https://www.bbc.co.uk/news/uk"));
    assert!(result_is_relevant(q, "axum - crates.io", "Ergonomic and modular web framework for Rust", "https://crates.io/crates/axum"));
    // a long query needs two content words, one is not enough
    assert!(!result_is_relevant("rust async runtime comparison benchmarks", "Rust news", "unrelated", "https://example.com"));
}

#[test]
fn low_risk_anchor_is_compact_high_risk_anchor_is_full() {
    let registry = ModelRegistry::new();
    let model = registry.lookup("claude-sonnet-5-5");
    let scanner = TemporalScanner::new();
    let date = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();

    let low = HorizonCalculator::evaluate(&model, &scanner.scan("rename this variable and add a unit test"), date);
    assert!(!low.requires_grounding);
    assert!(low.calibration_block.contains("[SYSTEM TEMPORAL CALIBRATION ANCHOR]"));
    assert!(low.calibration_block.contains("GROUNDING_MANDATORY: NO"));
    assert!(low.calibration_block.contains("CURRENT_EVALUATION_DATE: 2026-10-12"));
    assert!(!low.calibration_block.contains("INSTRUCTION:"));

    let high = HorizonCalculator::evaluate(&model, &scanner.scan("what is the latest version of react released in 2026"), date);
    assert!(high.requires_grounding);
    assert!(high.calibration_block.contains("GROUNDING_MANDATORY: YES"));
    assert!(high.calibration_block.contains("INSTRUCTION:"));
    assert!(low.calibration_block.len() * 2 < high.calibration_block.len(), "compact anchor should be well under half the size");
}
