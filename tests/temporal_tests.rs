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

#[test]
fn test_dossier_kv_cache_prefix_determinism() {
    use chronofact::cost::CostTracker;
    use chronofact::memory::{MemoryEngine, ProjectEntity};

    let mem = MemoryEngine::new_in_memory().unwrap();
    mem.upsert_entity(&ProjectEntity {
        id: "proj:axum".to_string(),
        project_id: "proj".to_string(),
        entity_name: "Axum".to_string(),
        entity_type: "TECH_STACK".to_string(),
        definition: "Axum 0.8 HTTP router".to_string(),
        version: "0.8.1".to_string(),
        updated_at: "2026-10-14T00:00:00Z".to_string(),
    })
    .unwrap();

    let d1 = mem.get_dossier("proj").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let d2 = mem.get_dossier("proj").unwrap();

    // Dossier markdown must be byte-identical across turns so KV prompt cache prefixes never bust
    assert_eq!(d1.dossier_markdown, d2.dossier_markdown);

    let p1 = CostTracker::build_cache_isolated_prompt("proj", &d1.dossier_markdown, "", "anchor1", "", "q1");
    let p2 = CostTracker::build_cache_isolated_prompt("proj", &d2.dossier_markdown, "", "anchor2", "", "q2");
    assert_eq!(p1.static_cache_prefix, p2.static_cache_prefix);
}

#[test]
fn test_claim_extractor_skips_fenced_code_and_extracts_blocks() {
    use chronofact::grounding::{ClaimExtractor, CodeInvariantChecker};

    let extractor = ClaimExtractor::new();
    let response = "Claude Sonnet 5.5 is the active 2026 Anthropic frontier model. For example, e.g. Node.js 22 supports native fetch.\n```typescript\nconst model = \"claude-3-5-sonnet-20241022\";\nconst sql = `SELECT * FROM users WHERE id = ${userId}`;\n```\nGPT-6 Astra is developed by OpenAI.";

    let claims = extractor.extract_claims(response);
    // Code lines inside ```typescript ... ``` must NOT be extracted as prose claims
    assert!(claims.iter().all(|c| !c.statement.contains("SELECT * FROM users")));
    assert!(claims.iter().any(|c| c.statement.contains("Node.js 22 supports native fetch")));

    // Fenced code blocks are extracted cleanly and audited by CodeInvariantChecker
    let blocks = extractor.extract_fenced_code_blocks(response);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].0, "typescript");

    let checker = CodeInvariantChecker::new();
    let audit = checker.audit_code("snippet.ts", &blocks[0].1, None);
    assert!(audit.violations.iter().any(|v| v.rule_id == "INV-TEMP-DEPRECATED-LIB"));
    assert!(audit.violations.iter().any(|v| v.rule_id == "INV-SEC-RAW-SQL"));
}

#[test]
fn test_mcplex_enhanced_router_camelcase_stems_and_spine_duo() {
    use chronofact::cost::{TfidfToolRouter, ToolCandidate};

    let router = TfidfToolRouter::default();
    let tools = vec![
        ToolCandidate::new("createPullRequestReview", "Submit a review on a pull request", "github", vec!["pullNumber".into(), "body".into()]),
        ToolCandidate::new("postgres_run_migration", "Run database schema migration on postgres", "db", vec!["sql".into()]),
        ToolCandidate::new("spine_adversarial_redteam", "Lumbar/Sacral adversarial architecture red-team", "spine", vec!["proposal".into()]),
        ToolCandidate::new("spine_verify_pushback", "Thoracic T1 pushback verification", "spine", vec!["challenge_prompt".into()]),
        ToolCandidate::new("weather_forecast", "Get weather forecast for city", "weather", vec!["city".into()]),
    ];

    // 1. camelCase splitting ("createPullRequestReview" matches "pull request review")
    let res1 = router.route("submit a pull request review on github", &tools, 2, None);
    assert_eq!(res1.selected_tools[0].tool.name, "createPullRequestReview");

    // 2. Morphological stem matching ("postgresql migrations" matches "postgres_run_migration")
    let res2 = router.route("apply postgresql migrations", &tools, 2, None);
    assert_eq!(res2.selected_tools[0].tool.name, "postgres_run_migration");

    // 3. SPINE duo intent routing ("redteam" -> spine_adversarial_redteam)
    let res3 = router.route("redteam this microservice architecture", &tools, 2, None);
    assert!(res3.selected_tools.iter().any(|s| s.tool.name == "spine_adversarial_redteam"));
}


