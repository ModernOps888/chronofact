use chrono::NaiveDate;
use chronofact::{HorizonCalculator, ModelRegistry, TemporalScanner};

#[test]
fn test_known_model_lookup_and_horizons() {
    let registry = ModelRegistry::new();
    let opus = registry.lookup("claude-opus-5-5");
    assert_eq!(opus.vendor, "Anthropic");
    assert_eq!(opus.official_knowledge_cutoff, NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());

    let grok47 = registry.lookup("grok 4.7");
    assert_eq!(grok47.vendor, "xAI");
    assert!(grok47.is_frontier);

    let sonnet = registry.lookup("claude-sonnet-5-5");
    assert_eq!(sonnet.vendor, "Anthropic");
    assert!(sonnet.is_frontier);

    let astra = registry.lookup("astr-6");
    assert_eq!(astra.vendor, "OpenAI");
    assert!(astra.display_name.contains("Astra 6"));
    assert!(astra.is_frontier);

    let sol = registry.lookup("sol 6.1");
    assert_eq!(sol.vendor, "OpenAI");
    assert!(sol.display_name.contains("Sol 6.1"));
    assert!(sol.is_frontier);

    let gemini = registry.lookup("gemini-3-8-flash");
    assert_eq!(gemini.vendor, "Google");
    assert!(gemini.is_frontier);

    let grok = registry.lookup("grok-3");
    assert_eq!(grok.vendor, "xAI");
    assert!(!grok.is_frontier); // Superseded by Grok 4 and Grok 4.7
    assert!(grok.display_name.contains("Grok 3"));

    let o3 = registry.lookup("o3-mini");
    assert_eq!(o3.vendor, "OpenAI");
    assert!(o3.is_frontier);

    let gpt5 = registry.lookup("gpt-5");
    assert_eq!(gpt5.vendor, "OpenAI");
    assert!(gpt5.is_frontier);
}

#[test]
fn test_temporal_scanner_detection() {
    let scanner = TemporalScanner::new();

    // Timeless query
    let timeless = scanner.scan("Explain the time complexity of quicksort algorithm");
    assert!(!timeless.is_temporally_sensitive);
    assert!(timeless.temporal_risk_score < 0.35);

    // Highly time-sensitive query
    let sensitive = scanner.scan("What are the latest breaking changes released in Next.js 16 and Claude Opus 5.5 in 2026?");
    assert!(sensitive.is_temporally_sensitive);
    assert!(sensitive.temporal_risk_score >= 0.70);
    assert!(sensitive.requires_search);
    assert!(sensitive.mentioned_years.contains(&2026));
    assert!(sensitive.search_query_suggestion.is_some());
}

#[test]
fn test_horizon_calculator_delta_and_calibration() {
    let registry = ModelRegistry::new();
    let scanner = TemporalScanner::new();

    let model = registry.lookup("gpt-4o");
    let scan = scanner.scan("What is the latest release of Rust in 2026?");
    let eval_date = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();

    let analysis = HorizonCalculator::evaluate(&model, &scan, eval_date);
    assert!(analysis.days_post_cutoff > 1000);
    assert!(analysis.requires_grounding);
    assert!(analysis.calibration_block.contains("[SYSTEM TEMPORAL CALIBRATION ANCHOR]"));
    assert!(analysis.calibration_block.contains("GROUNDING_MANDATORY: YES"));
}

#[test]
fn test_dynamic_rust_python_detection() {
    let scanner = TemporalScanner::new();

    // 1. Rust query with software intent
    let rust_scan = scanner.scan("How do I implement async handlers in axum with tokio?");
    assert!(rust_scan.is_temporally_sensitive);
    assert!(rust_scan.detected_entities.contains(&"axum".to_string()));
    assert!(rust_scan.detected_entities.contains(&"tokio".to_string()));
    assert!(rust_scan.requires_search);

    // 2. Python query with software intent
    let python_scan = scanner.scan("How to configure Pydantic with FastAPI?");
    assert!(python_scan.is_temporally_sensitive);
    assert!(python_scan.detected_entities.contains(&"pydantic".to_string()));
    assert!(python_scan.detected_entities.contains(&"fastapi".to_string()));

    // 3. Factual misattribution test for Astra 6 + Google DeepMind
    let astra_google = scanner.scan("Is Astra 6 developed by Google DeepMind?");
    assert!(!astra_google.outdated_models_flagged.is_empty());
    assert!(astra_google.outdated_models_flagged[0].contains("OpenAI models (launched Sep 2026), NOT Google DeepMind"));
}

