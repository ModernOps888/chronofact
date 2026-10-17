//! Phase 1 & Phase 2 Surgical Remediation Regression Test Suite
//!
//! Validates all architectural, token-economics, PhD mathematical/epistemic,
//! post-training flywheel (KTO/DPO), security, and grounding fixes.

use chronofact::cost::{TfidfToolRouter, ToolCandidate, ToolResponseCache};
use chronofact::grounding::claim::{AtomicClaim, ClaimCategory};
use chronofact::grounding::{FactVerifier, NeuralNliCalibrator, VerificationStatus};
use chronofact::memory::{
    EmbeddingProvider, EpistemicTrajectory, MemoryCalibrator, MemoryEngine, PostTrainingPair,
    ProjectEntity,
};
use chronofact::research::{PackageMetadata, SourceChunk};
use chronofact::temporal::TemporalScanner;
use tempfile::tempdir;

#[test]
fn test_cache_invalidate_server_supports_slash_fqn() {
    let cache = ToolResponseCache::new(60, 100);
    cache.put(
        "github/list_PRs",
        &serde_json::json!({"repo": "chronofact"}),
        serde_json::json!({"items": []}),
    );
    cache.put(
        "postgres/query",
        &serde_json::json!({"sql": "SELECT 1"}),
        serde_json::json!({"rows": [1]}),
    );

    cache.invalidate_server("github");
    assert!(cache
        .get("github/list_PRs", &serde_json::json!({"repo": "chronofact"}))
        .is_none());
    assert!(cache
        .get("postgres/query", &serde_json::json!({"sql": "SELECT 1"}))
        .is_some());
}

#[test]
fn test_memory_calibrator_small_sample_and_bessel_correction() {
    // Small sample <= 3 returns floor directly without z-score overshoot
    let small = vec![0.10, 0.90];
    let thresh_small = MemoryCalibrator::calculate_adaptive_threshold(
        &small,
        EmbeddingProvider::LexicalTokenOverlap,
        0.5,
    );
    assert!(
        (thresh_small - 0.15).abs() < 1e-6,
        "Small sample (n <= 3) must return floor 0.15, got {}",
        thresh_small
    );

    // Sample n = 4 uses Bessel's sample std dev (n - 1)
    let sample = vec![0.20, 0.40, 0.60, 0.80];
    let thresh = MemoryCalibrator::calculate_adaptive_threshold(
        &sample,
        EmbeddingProvider::LexicalTokenOverlap,
        0.5,
    );
    // Mean = 0.50, sample variance = (0.09 + 0.01 + 0.01 + 0.09) / 3 = 0.20 / 3 = 0.0666667
    // std_dev = sqrt(0.0666667) = 0.25819889, mean + 0.5 * std_dev = 0.6290994
    assert!(
        (thresh - 0.6290994).abs() < 1e-4,
        "Expected Bessel-corrected threshold ~0.6291, got {}",
        thresh
    );
}

#[test]
fn test_dossier_preserves_custom_entity_types_and_suppresses_empty_shell() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_dossier_remediation.db");
    let memory = MemoryEngine::open(&db_path).unwrap();

    memory
        .upsert_entity(&ProjectEntity {
            id: "p1:dm".to_string(),
            project_id: "p1".to_string(),
            entity_name: "OrderLedgerSchema".to_string(),
            entity_type: "DATA_MODEL".to_string(),
            definition: "Immutable double-entry ledger table with strict foreign keys".to_string(),
            version: "2.1.0".to_string(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        })
        .unwrap();

    let full = memory.get_dossier("p1").unwrap();
    assert!(
        full.architectural_invariants
            .iter()
            .any(|s: &String| s.contains("OrderLedgerSchema")),
        "DATA_MODEL entity must not be dropped during dossier compilation"
    );

    // Unrelated query must return an empty dossier_markdown string (zero token bloat)
    let unrelated = memory
        .get_relevant_dossier("p1", "How to animate SVG icons in CSS keyframes")
        .unwrap();
    assert!(
        unrelated.dossier_markdown.is_empty(),
        "Zero-match query must not emit an empty XML shell, got: {}",
        unrelated.dossier_markdown
    );
}

#[test]
fn test_post_training_kto_pairs_and_epistemic_trajectory_persistence() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_kto_flywheel.db");
    let memory = MemoryEngine::open(&db_path).unwrap();

    memory
        .record_epistemic_trajectory(&EpistemicTrajectory {
            id: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
            project_id: "proj-ml".to_string(),
            model_id: "openai/gpt-6-astra".to_string(),
            query: "Verify port configuration and TLS invariants".to_string(),
            draft_response: "Server listens on port 8080".to_string(),
            verified_claims_json: "[]".to_string(),
            contradicted_claims_json: "[\"Server listens on port 8080\"]".to_string(),
            unverified_claims_json: "[]".to_string(),
            source_evidence_json: "[\"Server listens on port 3030\"]".to_string(),
            hallucination_risk_index: 0.50,
            intervention_type: "CLAIM_CONTRADICTION_BLOCK".to_string(),
        })
        .unwrap();

    memory
        .record_post_training_pair(&PostTrainingPair {
            id: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
            project_id: "proj-ml".to_string(),
            prompt: "Port configuration check".to_string(),
            response: "Server listens on port 8080".to_string(),
            label: "UNDESIRABLE".to_string(),
            kto_weight: 2.25,
            source_evidence: "Server listens on port 3030".to_string(),
            reward_score: -1.0,
        })
        .unwrap();

    let trajectories = memory.get_epistemic_trajectories(10).unwrap();
    assert_eq!(trajectories.len(), 1);
    assert_eq!(
        trajectories[0].intervention_type,
        "CLAIM_CONTRADICTION_BLOCK"
    );

    let kto_pairs = memory.get_post_training_pairs(10).unwrap();
    assert_eq!(kto_pairs.len(), 1);
    assert_eq!(kto_pairs[0].label, "UNDESIRABLE");
    assert!((kto_pairs[0].kto_weight - 2.25).abs() < 1e-5);
}

#[test]
fn test_temporal_scanner_comparison_clause_and_year_differentiation() {
    let scanner = TemporalScanner::new();

    // Legitimate cross-vendor comparison must not trigger false vendor misattribution
    let cmp_scan = scanner.scan("Compare Anthropic Claude Opus 5.5 vs OpenAI GPT-6 Astra");
    assert!(
        !cmp_scan
            .outdated_models_flagged
            .iter()
            .any(|w| w.contains("VENDOR MISATTRIBUTION")),
        "Cross-vendor comparison must not trigger false vendor misattribution: {:?}",
        cmp_scan.outdated_models_flagged
    );

    // Year differentiation: 2026 (>2024) has higher temporal risk than 2024
    let scan_2024 = scanner.scan("Review the system specification from 2024");
    let scan_2026 = scanner.scan("Review the system specification from 2026");
    assert!(
        scan_2026.temporal_risk_score > scan_2024.temporal_risk_score,
        "2026 risk ({}) must exceed 2024 risk ({})",
        scan_2026.temporal_risk_score,
        scan_2024.temporal_risk_score
    );
}

#[test]
fn test_tfidf_router_stem_downweighting_and_cache_anchor() {
    let router = TfidfToolRouter::new(0.05);
    let tools = vec![
        ToolCandidate::new(
            "database_admin",
            "Manage database connections and schemas",
            "db",
            vec!["connection".into()],
        ),
        ToolCandidate::new(
            "databank_archive",
            "Archive historical databank records",
            "storage",
            vec!["archive".into()],
        ),
    ];

    let res = router.route("Configure database connection", &tools, 1, None);
    assert_eq!(res.selected_tools.len(), 1);
    assert_eq!(
        res.selected_tools[0].tool.name, "database_admin",
        "Exact word match 'database' must outrank 5-char stem collision 'databank'"
    );
    assert!(!res.prompt_cache_anchor.is_empty());
    assert!((res.cost_reduction_pct - res.savings_percentage).abs() < 1e-5);
}

#[test]
fn test_verifier_word_boundary_prevents_substring_false_entailment() {
    let verifier = FactVerifier::new();
    let claims = vec![AtomicClaim {
        id: "c-sub".to_string(),
        statement: "Use act pro api in production.".to_string(),
        category: ClaimCategory::TechnicalApi,
    }];
    // Source contains "react", "project", "capital", "reproduction" (superstrings of act, pro, api, production)
    let sources = vec![SourceChunk {
        id: "src-super".to_string(),
        url: "https://example.com".to_string(),
        title: "Unrelated text".to_string(),
        content: "React project capital reproduction.".to_string(),
        integrity_hash: "h".to_string(),
        is_sanitized: true,
    }];

    let report = verifier.verify_claims(&claims, &sources);
    assert_ne!(
        report.claims[0].status,
        VerificationStatus::Entailed,
        "Substring collisions inside unrelated words must not trigger Entailed"
    );
}

#[test]
fn test_package_metadata_ssrf_url_sanitization() {
    let malicious = PackageMetadata {
        name: "evil-crate".into(),
        ecosystem: "crates.io".into(),
        latest_version: "1.0.0".into(),
        description: "test crate".into(),
        homepage: Some("http://169.254.169.254/latest/meta-data/iam".into()),
        repository: Some("http://127.0.0.1:8080/internal-admin".into()),
        documentation: Some("http://localhost:3000/secrets".into()),
        deprecated: None,
        published: Some("2026-10-01".into()),
        license: Some("MIT".into()),
        retrieved_at: "now".into(),
    };

    let chunk = malicious.to_source_chunk();
    assert_eq!(
        chunk.url, "https://crates.io/crates/evil-crate",
        "SSRF URLs in package metadata must fall back to canonical registry URL"
    );
    assert!(!chunk.content.contains("169.254.169.254"));
    assert!(!chunk.content.contains("127.0.0.1"));
    assert!(!chunk.content.contains("localhost"));

    // Verify NeuralNliCalibrator temperature scaling
    let cal = NeuralNliCalibrator::calibrate_confidence(0.90, 0.85);
    assert!(cal > 0.90 && cal < 1.0);
}
