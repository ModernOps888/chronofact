use chronofact::{
    CostTracker, TfidfToolRouter, ToolCandidate, ToolResponseCache,
};
use serde_json::json;

#[test]
fn test_tfidf_tool_routing_prunes_irrelevant_tools_and_saves_tokens() {
    let router = TfidfToolRouter::default();

    let tools = vec![
        ToolCandidate::new(
            "search_web",
            "Performs real-time web search for frontier technical topics",
            "antigravity",
            vec!["query".into(), "max_results".into()],
        ),
        ToolCandidate::new(
            "read_url_content",
            "Fetches raw html markdown from a web url",
            "antigravity",
            vec!["url".into()],
        ),
        ToolCandidate::new(
            "view_file",
            "Reads file contents from disk filesystem",
            "antigravity",
            vec!["path".into(), "offset".into()],
        ),
        ToolCandidate::new(
            "replace_file_content",
            "Modifies source code in an existing file on disk",
            "antigravity",
            vec!["file".into(), "target".into(), "replacement".into()],
        ),
        ToolCandidate::new(
            "run_command",
            "Executes a powershell command in terminal",
            "antigravity",
            vec!["command".into()],
        ),
        ToolCandidate::new(
            "database_migrate",
            "Runs postgres sql migration scripts against database",
            "db_tools",
            vec!["migration_file".into()],
        ),
        ToolCandidate::new(
            "oauth_token_refresh",
            "Refreshes JWT oauth2 security tokens",
            "auth_service",
            vec!["client_id".into(), "refresh_token".into()],
        ),
    ];

    // Query specifically about web search
    let search_query = "Search online documentation for the latest release notes";
    let result = router.route(search_query, &tools, 2, None);

    assert!(result.selected_tools.len() >= 1 && result.selected_tools.len() <= 2);
    // The top selected tool must be search_web
    let top_tool = &result.selected_tools[0].tool.name;
    assert_eq!(top_tool, "search_web");

    // Verify significant token savings
    assert!(result.tokens_saved > 0);
    assert!(result.pruned_tools >= 5);
    assert!(
        result.savings_percentage >= 50.0,
        "Savings percentage was only {:.1}%",
        result.savings_percentage
    );
}

#[test]
fn test_idempotent_tool_response_cache_hit_and_eviction() {
    let cache = ToolResponseCache::new(60, 10);

    let tool_name = "chronofact_temporal_check";
    let args_1 = json!({ "model_id": "claude-sonnet-5-5", "query": "rust 1.85 async closures" });
    let val_1 = json!({ "status": "entailed", "delta": 204 });

    // 1. Initial lookup -> Cache miss
    assert!(cache.get(tool_name, &args_1).is_none());

    // 2. Put into cache
    cache.put(tool_name, &args_1, val_1.clone());

    // 3. Second lookup -> Cache hit
    let cached = cache.get(tool_name, &args_1).expect("expected cache hit");
    assert_eq!(cached, val_1);

    // 4. Different arguments -> Cache miss
    let args_2 = json!({ "model_id": "claude-sonnet-5-5", "query": "deepseek v4.1" });
    assert!(cache.get(tool_name, &args_2).is_none());

    // 5. Verify stats
    let (hits, misses, entries, hit_rate) = cache.stats();
    assert_eq!(hits, 1);
    assert_eq!(misses, 2);
    assert_eq!(entries, 1);
    assert!(hit_rate > 30.0);
}

#[test]
fn test_cost_tracker_metrics_and_usd_calculation() {
    let tracker = CostTracker::new(0.000003); // $3 per million tokens

    // Record 10 queries that each pruned 5 tools (saving 1,000 tokens per query)
    for _ in 0..10 {
        tracker.record_savings(5, 1000);
    }

    let metrics = tracker.get_metrics(8, 2, 1);

    assert_eq!(metrics.total_queries_optimized, 10);
    assert_eq!(metrics.total_tools_pruned, 50);
    assert_eq!(metrics.total_tokens_saved, 10_000);
    assert_eq!(metrics.cache_hit_rate_pct, 80.0);
    assert_eq!(metrics.average_tokens_saved_per_turn, 1000.0);
    assert!(metrics.estimated_usd_saved >= 0.03);

    // Check prompt cache anchor generation
    let anchor = CostTracker::generate_cache_aligned_prefix("my-test-project");
    assert!(anchor.contains("<prompt_cache_anchor project=\"my-test-project\""));
}

#[test]
fn test_idiomatic_build_failing_query_retains_terminal_and_file_tools() {
    let router = TfidfToolRouter::default();

    let tools = vec![
        ToolCandidate::new("search_web", "Performs real-time web search for frontier technical topics", "antigravity", vec![]),
        ToolCandidate::new("terminal_exec", "Executes shell commands in a terminal process", "core", vec![]),
        ToolCandidate::new("read_file", "Reads file contents from disk filesystem", "core", vec![]),
        ToolCandidate::new("database_migrate", "Runs postgres sql migration scripts", "db", vec![]),
        ToolCandidate::new("oauth_token_refresh", "Refreshes JWT oauth2 security tokens", "auth", vec![]),
    ];

    // Idiomatic query with zero unigram overlap with "terminal" or "read" or "file"
    let query = "Why is the build failing with error code 127?";
    let result = router.route(query, &tools, 3, None);

    let selected_names: Vec<&str> = result.selected_tools.iter().map(|s| s.tool.name.as_str()).collect();
    assert!(
        selected_names.contains(&"terminal_exec") || selected_names.contains(&"read_file"),
        "Intent-boosted dictionary must retain terminal or file tools on build/failing/error: got {:?}",
        selected_names
    );
}

#[test]
fn test_always_retained_pinning_guarantees_inclusion() {
    let router = TfidfToolRouter::default()
        .with_always_retained(&["terminal_exec", "read_file"]);

    let tools = vec![
        ToolCandidate::new("search_web", "Performs web search", "web", vec![]),
        ToolCandidate::new("terminal_exec", "Executes shell commands", "core", vec![]),
        ToolCandidate::new("read_file", "Reads file contents", "core", vec![]),
        ToolCandidate::new("database_migrate", "Database migrations", "db", vec![]),
    ];

    // Even with a very high threshold (0.95) and unrelated query
    let query = "Look up weather in London";
    let result = router.route(query, &tools, 3, Some(0.95));

    let selected_names: Vec<&str> = result.selected_tools.iter().map(|s| s.tool.name.as_str()).collect();
    assert!(selected_names.contains(&"terminal_exec"));
    assert!(selected_names.contains(&"read_file"));
}

#[test]
fn test_fallback_semantic_gating_when_zero_intersection() {
    let router = TfidfToolRouter::default();

    let tools = vec![
        ToolCandidate::new("tool_alpha", "Does arbitrary operation alpha", "srv", vec![]),
        ToolCandidate::new("tool_beta", "Does arbitrary operation beta", "srv", vec![]),
        ToolCandidate::new("tool_gamma", "Does arbitrary operation gamma", "srv", vec![]),
    ];

    // Query with zero unigram or intent overlap
    let query = "qwerty asdf zxcvbnm";
    let result = router.route(query, &tools, 2, Some(0.80));

    // Fallback must retain top_k tools rather than leaving the agent blind
    assert_eq!(result.selected_tools.len(), 2);
}

#[test]
fn test_memory_calibrator_percentile_ranking() {
    use chronofact::MemoryCalibrator;

    let scores = vec![0.10, 0.35, 0.50, 0.72, 0.88, 0.95];
    let p70 = MemoryCalibrator::calculate_percentile_threshold(&scores, 0.70);
    assert!(p70 >= 0.70 && p70 <= 0.88);

    let items = vec![
        ("item_1", 0.10),
        ("item_2", 0.35),
        ("item_3", 0.50),
        ("item_4", 0.72),
        ("item_5", 0.88),
        ("item_6", 0.95),
    ];
    let ranked = MemoryCalibrator::filter_by_percentile(items, 0.60, 3);
    assert!(!ranked.is_empty());
    assert!(ranked.len() <= 3);
    assert_eq!(ranked[0].0, "item_6");
}

#[test]
fn test_critical_tool_pinning_whitelist_and_route_tools_functional_api() {
    use chronofact::{route_tools, ToolSchema};

    let tools = vec![
        ToolSchema::new("read_file", "Reads file contents from disk", "core", vec![]),
        ToolSchema::new("write_file", "Writes file contents to disk", "core", vec![]),
        ToolSchema::new("terminal_exec", "Executes shell commands in a terminal", "core", vec![]),
        ToolSchema::new("unrelated_calculator", "Computes arithmetic numbers", "math", vec![]),
        ToolSchema::new("unrelated_weather", "Fetches weather forecasts", "geo", vec![]),
    ];

    // Idiomatic build failure prompt with 0.45 threshold
    let query = "Why is the build failing with error code 127 in this container?";
    let selected = route_tools(query, &tools, 0.45);

    let names: Vec<&str> = selected.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"read_file"), "read_file must be retained by ALWAYS_RETAINED whitelist");
    assert!(names.contains(&"terminal_exec"), "terminal_exec must be retained by ALWAYS_RETAINED whitelist");
    assert!(!names.contains(&"unrelated_calculator"), "unrelated calculator must be pruned");
}

#[test]
fn test_calibrated_quantile_memory_gating_is_memory_relevant() {
    use chronofact::{is_memory_relevant, EmbeddingProfile, EmbeddingProvider};

    let bge_profile = EmbeddingProfile::with_threshold(EmbeddingProvider::BgeLarge, 0.68);
    let openai_profile = EmbeddingProfile::with_threshold(EmbeddingProvider::OpenAiTextEmbedding3, 0.76);

    // Score of 0.70 is relevant for BGE (>= 0.68) but irrelevant for OpenAI Text-3 (needs >= 0.76)
    assert!(is_memory_relevant(0.70, &bge_profile));
    assert!(!is_memory_relevant(0.70, &openai_profile));

    assert!(is_memory_relevant(0.80, &openai_profile));
}

#[test]
fn test_conceptual_query_suppresses_operational_tool_over_retention() {
    use chronofact::{is_conceptual_or_abstract_query, route_tools, TfidfToolRouter, ToolCandidate, ToolSchema};

    let conceptual_query = "Explain how the Rust borrow checker handles lifetimes during compilation";
    assert!(
        is_conceptual_or_abstract_query(conceptual_query),
        "Conceptual query must be detected"
    );

    let diagnostic_query = "Why is the build failing with error code 127 in this container?";
    assert!(
        !is_conceptual_or_abstract_query(diagnostic_query),
        "Diagnostic query must NOT be detected as conceptual"
    );

    let tools = vec![
        ToolSchema::new("read_file", "Reads file contents from disk", "core", vec![]),
        ToolSchema::new("terminal_exec", "Executes shell commands in a terminal", "core", vec![]),
        ToolSchema::new("search_web", "Performs web search for technical documentation", "web", vec![]),
        ToolSchema::new("unrelated_calculator", "Computes arithmetic numbers", "math", vec![]),
    ];

    // Under conceptual query, operational primitives (terminal_exec) are pruned
    let conceptual_selected = route_tools(conceptual_query, &tools, 0.45);
    let conceptual_names: Vec<&str> = conceptual_selected.iter().map(|t| t.name.as_str()).collect();
    assert!(
        !conceptual_names.contains(&"terminal_exec"),
        "terminal_exec must be pruned for conceptual query"
    );

    // Under diagnostic query, operational primitives are strictly retained
    let diagnostic_selected = route_tools(diagnostic_query, &tools, 0.45);
    let diagnostic_names: Vec<&str> = diagnostic_selected.iter().map(|t| t.name.as_str()).collect();
    assert!(
        diagnostic_names.contains(&"terminal_exec"),
        "terminal_exec must be retained for diagnostic query"
    );
    assert!(
        diagnostic_names.contains(&"read_file"),
        "read_file must be retained for diagnostic query"
    );

    // Also verify router.route() with intent dictionary suppression
    let router = TfidfToolRouter::default();
    let candidates = vec![
        ToolCandidate::new("terminal_exec", "Executes shell commands in a terminal", "core", vec![]),
        ToolCandidate::new("search_web", "Performs web search for technical documentation", "web", vec![]),
    ];
    let route_result = router.route(conceptual_query, &candidates, 2, Some(0.45));
    // Intent boost for 'compilation' is suppressed, so terminal_exec is pruned
    let routed_names: Vec<&str> = route_result.selected_tools.iter().map(|t| t.tool.name.as_str()).collect();
    assert!(
        !routed_names.contains(&"terminal_exec"),
        "router.route() must suppress intent boost on conceptual queries"
    );
}

#[test]
fn test_cache_isolated_prompt_prefix_stability() {
    use chronofact::CostTracker;

    let project_id = "tenant-enterprise-42";
    let static_system = "You are ChronoFact Invariant Auditor. Follow all L3 reality invariants.";
    let static_schemas = "{\"tools\": [{\"name\": \"verify_claims\"}, {\"name\": \"ground_query\"}]}";

    // Turn 1: Initial user turn
    let dynamic_anchor_turn_1 = "<chronofact_temporal_anchor>\nCURRENT_DATE: 2026-10-07\nDELTA: +248\n</chronofact_temporal_anchor>";
    let dynamic_evidence_turn_1 = "Evidence 1: Rust 1.85 released async closures in 2025.";
    let query_turn_1 = "Explain async closures in Rust 1.85";

    let prompt_turn_1 = CostTracker::build_cache_isolated_prompt(
        project_id,
        static_system,
        static_schemas,
        dynamic_anchor_turn_1,
        dynamic_evidence_turn_1,
        query_turn_1,
    );

    // Turn 2: Subsequent user turn (new anchor calculation, new evidence, new query)
    let dynamic_anchor_turn_2 = "<chronofact_temporal_anchor>\nCURRENT_DATE: 2026-10-08\nDELTA: +249\n</chronofact_temporal_anchor>";
    let dynamic_evidence_turn_2 = "Evidence 2: GPT-6 Astra released by OpenAI.";
    let query_turn_2 = "What provider released GPT-6 Astra?";

    let prompt_turn_2 = CostTracker::build_cache_isolated_prompt(
        project_id,
        static_system,
        static_schemas,
        dynamic_anchor_turn_2,
        dynamic_evidence_turn_2,
        query_turn_2,
    );

    // 1. Static Cache Prefix MUST BE 100% IDENTICAL byte-for-byte across turns
    assert_eq!(
        prompt_turn_1.static_cache_prefix,
        prompt_turn_2.static_cache_prefix,
        "Static cache prefix must be byte-for-byte identical across turns to guarantee prompt cache hits"
    );

    // 2. Cache boundary marker must be present
    assert_eq!(prompt_turn_1.cache_boundary_marker, "\n<!-- CACHE_BOUNDARY_EPHEMERAL -->\n");

    // 3. Dynamic content must only exist in dynamic_context_suffix
    assert_ne!(prompt_turn_1.dynamic_context_suffix, prompt_turn_2.dynamic_context_suffix);
    assert!(!prompt_turn_1.static_cache_prefix.contains("2026-10-07"));
    assert!(!prompt_turn_2.static_cache_prefix.contains("2026-10-08"));
    assert!(prompt_turn_1.dynamic_context_suffix.contains("2026-10-07"));
    assert!(prompt_turn_2.dynamic_context_suffix.contains("2026-10-08"));
    assert!(prompt_turn_1.cache_aligned);
}

#[test]
fn test_sqlite_with_busy_retry_concurrency() {
    use chronofact::{MemoryEngine, SessionSummary};
    use std::sync::Arc;
    use std::thread;

    let temp_dir = std::env::temp_dir().join(format!("chronofact_busy_test_{}.db", std::process::id()));
    let engine = Arc::new(MemoryEngine::open(&temp_dir).expect("Failed to open test database"));

    let num_threads = 6;
    let ops_per_thread = 20;
    let mut handles = Vec::new();

    for t_idx in 0..num_threads {
        let eng = Arc::clone(&engine);
        let handle = thread::spawn(move || {
            for i in 0..ops_per_thread {
                let session = SessionSummary {
                    session_id: format!("sess_{}_{}", t_idx, i),
                    project_id: format!("proj_{}", t_idx),
                    created_at: "2026-10-07T12:00:00Z".to_string(),
                    summary: format!("Session {} from thread {}", i, t_idx),
                };
                eng.record_session(&session).expect("record_session should succeed under retry");
                eng.record_cost_event("test query", 3, 150, 0.00045)
                    .expect("record_cost_event should succeed under retry");
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("Thread panicked");
    }

    let (queries, pruned, saved, usd) = engine.get_cost_totals().expect("Failed to get totals");
    assert_eq!(queries, (num_threads * ops_per_thread) as u64);
    assert_eq!(pruned, (num_threads * ops_per_thread * 3) as u64);
    assert_eq!(saved, (num_threads * ops_per_thread * 150) as u64);
    assert!(usd > 0.0);

    let _ = std::fs::remove_file(&temp_dir);
}

#[test]
fn test_tool_response_cache_fs_metadata_and_path_invalidation() {
    use std::io::Write;

    let cache = ToolResponseCache::new(60, 10);
    let temp_file = std::env::temp_dir().join(format!("test_cache_file_{}.txt", std::process::id()));

    // 1. Create file with initial content
    {
        let mut f = std::fs::File::create(&temp_file).expect("Failed to create temp file");
        f.write_all(b"initial file content").expect("Failed to write temp file");
    }

    let path_str = temp_file.to_str().unwrap();
    let args = json!({ "path": path_str });
    let tool_name = "read_file";

    // 2. Put initial cached value
    cache.put(tool_name, &args, json!({ "content": "initial file content" }));

    // Verify cache hit
    let hit = cache.get(tool_name, &args);
    assert!(hit.is_some(), "Expected cache hit for unchanged file");

    // 3. Mutate file on disk (sleep briefly to guarantee distinct mtime)
    std::thread::sleep(std::time::Duration::from_millis(50));
    {
        let mut f = std::fs::File::create(&temp_file).expect("Failed to recreate temp file");
        f.write_all(b"modified and updated file content with different length").expect("Failed to write updated content");
    }

    // 4. Cache get with same args must yield a MISS because mtime/size changed in the computed key
    let stale_check = cache.get(tool_name, &args);
    assert!(stale_check.is_none(), "Cache must MISS after underlying file was modified on disk");

    // 5. Test explicit invalidate_path
    cache.put(tool_name, &args, json!({ "content": "modified and updated file content" }));
    assert!(cache.get(tool_name, &args).is_some());
    cache.invalidate_path(path_str);
    assert!(cache.get(tool_name, &args).is_none(), "invalidate_path must evict entries referencing file");

    // 6. Test invalidate_filesystem_entries
    cache.put(tool_name, &args, json!({ "content": "modified and updated file content" }));
    assert!(cache.get(tool_name, &args).is_some());
    cache.invalidate_filesystem_entries();
    assert!(cache.get(tool_name, &args).is_none(), "invalidate_filesystem_entries must evict all file entries");

    let _ = std::fs::remove_file(&temp_file);
}

#[test]
fn test_tool_starvation_recovery_and_foundational_pins() {
    let router = TfidfToolRouter::default().with_foundational_pins();

    let tools = vec![
        ToolCandidate::new("search_web", "Performs web search", "antigravity", vec!["query".into()]),
        ToolCandidate::new("read_url_content", "Fetches url markdown", "antigravity", vec!["url".into()]),
        ToolCandidate::new("terminal_exec", "Executes terminal commands", "antigravity", vec!["cmd".into()]),
        ToolCandidate::new("database_migrate", "Runs sql migrations", "db", vec!["file".into()]),
        ToolCandidate::new("chronofact_expand_tool_palette", "Recovers and injects additional tool schemas on demand during multi-step execution", "chronofact", vec!["query_or_category".into()]),
        ToolCandidate::new("gateway_find_tools", "Searches across all upstream servers", "chronofact-gateway", vec!["query".into()]),
    ];

    // Narrow query: "search web documentation"
    let result = router.route("search web documentation", &tools, 2, None);

    // Foundational recovery tools MUST be retained even when top_k is 2
    let retained_names: Vec<&str> = result.selected_tools.iter().map(|t| t.tool.name.as_str()).collect();
    assert!(retained_names.contains(&"search_web"));
    assert!(
        retained_names.contains(&"chronofact_expand_tool_palette"),
        "chronofact_expand_tool_palette must be pinned to prevent agent starvation in multi-hop tasks"
    );
    assert!(
        retained_names.contains(&"gateway_find_tools"),
        "gateway_find_tools must be pinned to allow tool discovery"
    );

    // Now test recovery: simulate mid-task expansion for database migration
    let recovery_result = router.route("database postgres sql migration", &tools, 2, None);
    let recovered_names: Vec<&str> = recovery_result.selected_tools.iter().map(|t| t.tool.name.as_str()).collect();
    assert!(recovered_names.contains(&"database_migrate"), "Tool palette expansion must successfully recover secondary tools");
}

#[test]
fn test_adaptive_memory_two_tier_gating_rejects_noise_cluster() {
    use chronofact::{EmbeddingProvider, MemoryCalibrator};

    // Low-similarity noise cluster (all between 0.10 and 0.22)
    let noise_cluster: Vec<(String, f32)> = vec![
        ("item_a".into(), 0.11),
        ("item_b".into(), 0.14),
        ("item_c".into(), 0.18),
        ("item_d".into(), 0.22),
    ];

    // Filter using BgeLarge (floor: 0.55) or CohereV3 (floor: 0.45)
    let filtered_noise = MemoryCalibrator::filter_adaptive(
        noise_cluster,
        EmbeddingProvider::CohereV3,
        0.5,
        10,
    );

    assert!(
        filtered_noise.is_empty(),
        "Two-tier adaptive gating must reject an entire cluster of low-similarity noise when below absolute baseline floor"
    );

    // Mixed distribution: noise items (0.10, 0.20) + relevant items (0.65, 0.88)
    let mixed_cluster: Vec<(String, f32)> = vec![
        ("noise_1".into(), 0.10),
        ("noise_2".into(), 0.20),
        ("good_1".into(), 0.65),
        ("good_2".into(), 0.88),
    ];

    let filtered_mixed = MemoryCalibrator::filter_adaptive(
        mixed_cluster,
        EmbeddingProvider::CohereV3,
        -1.0,
        10,
    );

    assert_eq!(filtered_mixed.len(), 2, "Only items passing both absolute floor and dynamic threshold should be retained");
    let kept_names: Vec<String> = filtered_mixed.into_iter().map(|(name, _)| name).collect();
    assert!(kept_names.contains(&"good_1".to_string()));
    assert!(kept_names.contains(&"good_2".to_string()));
}

#[test]
fn test_cache_isolated_prompt_prefix_floor_guarantee() {
    use chronofact::{CostTracker, PROMPT_CACHE_MINIMUM_TOKEN_FLOOR};

    let project_id = "tenant-enterprise-floor-test";
    let static_system = "You are ChronoFact Invariant Auditor. Follow all L3 reality invariants.";
    let static_schemas = "{\"tools\": [{\"name\": \"verify_claims\"}]}"; // small schema ~100 tokens

    // Turn 1
    let prompt_turn_1 = CostTracker::build_cache_isolated_prompt(
        project_id,
        static_system,
        static_schemas,
        "<chronofact_temporal_anchor>\nCURRENT_DATE: 2026-10-07\n</chronofact_temporal_anchor>",
        "Evidence 1: Verified in 2026.",
        "Verify system state",
    );

    // Turn 2: Different dynamic inputs
    let prompt_turn_2 = CostTracker::build_cache_isolated_prompt(
        project_id,
        static_system,
        static_schemas,
        "<chronofact_temporal_anchor>\nCURRENT_DATE: 2026-10-08\n</chronofact_temporal_anchor>",
        "Evidence 2: Different evidence chunk.",
        "Check another query",
    );

    // 1. Static prefix estimated tokens must clear the 1,088 floor
    assert!(
        prompt_turn_1.static_prefix_estimated_tokens >= PROMPT_CACHE_MINIMUM_TOKEN_FLOOR,
        "Static prefix must clear provider prompt caching floor (>= 1088), got {}",
        prompt_turn_1.static_prefix_estimated_tokens
    );
    assert_eq!(
        prompt_turn_1.static_prefix_estimated_tokens,
        prompt_turn_2.static_prefix_estimated_tokens
    );

    // 2. Standardized context rules must be injected to bridge deficit
    assert!(prompt_turn_1.static_cache_prefix.contains("[STANDARDIZED_PROJECT_CACHE_CONTEXT]"));
    assert!(prompt_turn_1.static_cache_prefix.contains("PROVIDER_CACHE_POLICY: MIN_FLOOR_1088_TOKENS"));

    // 2b. Test StatelessSingleShot does NOT pad artificially
    let single_shot = CostTracker::assemble_cache_aligned_prompt_with_mode(
        project_id,
        static_system,
        static_schemas,
        "<chronofact_temporal_anchor>\nCURRENT_DATE: 2026-10-07\n</chronofact_temporal_anchor>",
        "Evidence: minimal chunk.",
        "One-shot CI query",
        chronofact::cost::tracker::SessionMode::StatelessSingleShot,
    );
    assert!(!single_shot.cache_aligned);
    assert!(!single_shot.static_cache_prefix.contains("[STANDARDIZED_PROJECT_CACHE_CONTEXT]"));
    assert!(single_shot.static_prefix_estimated_tokens < PROMPT_CACHE_MINIMUM_TOKEN_FLOOR);

    // 3. Static prefix must remain 100% byte-for-byte identical across turns
    assert_eq!(
        prompt_turn_1.static_cache_prefix,
        prompt_turn_2.static_cache_prefix,
        "Static cache prefix must be byte-stable across multi-turn interactions"
    );

    // 4. Dynamic suffixes must differ
    assert_ne!(prompt_turn_1.dynamic_context_suffix, prompt_turn_2.dynamic_context_suffix);
}

#[test]
fn test_route_with_token_floor_enforcement() {
    use chronofact::{route_tools_with_token_floor, TfidfToolRouter, ToolCandidate, ToolSchema};

    let router = TfidfToolRouter::default();

    let tools = vec![
        ToolCandidate::new("search_web", "Performs real-time web search for technical facts", "web", vec!["query".into()]),
        ToolCandidate::new("read_url_content", "Fetches raw html markdown from a web url", "web", vec!["url".into()]),
        ToolCandidate::new("extract_metrics", "Extracts runtime latency metrics from benchmarks", "analytics", vec!["id".into()]),
        ToolCandidate::new("analyze_performance", "Analyzes system memory and cpu bottlenecks", "analytics", vec!["proc".into()]),
        ToolCandidate::new("database_vacuum", "Vacuums dead rows from sqlite database", "storage", vec!["table".into()]),
    ];

    // Standard route with top_k = 1 would select 1 tool (~180 tokens)
    let base_res = router.route("search web documentation", &tools, 1, None);
    assert_eq!(base_res.selected_tools.len(), 1);
    assert!(base_res.tokens_after < 500);

    // Route with token floor = 500 tokens
    let floor_res = router.route_with_token_floor("search web documentation", &tools, 1, 500, None);
    assert!(
        floor_res.tokens_after >= 500,
        "route_with_token_floor must retain additional tools to satisfy token floor, got {}",
        floor_res.tokens_after
    );
    assert!(floor_res.selected_tools.len() >= 3);

    // Functional API test
    let schemas: Vec<ToolSchema> = tools.clone();
    let selected_functional = route_tools_with_token_floor("search web documentation", &schemas, 0.1, 500);
    let functional_tokens: usize = selected_functional.iter().map(|s| s.estimated_tokens).sum();
    assert!(
        functional_tokens >= 500,
        "route_tools_with_token_floor must retain tools to clear floor, got {}",
        functional_tokens
    );
}


