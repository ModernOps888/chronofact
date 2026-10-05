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
