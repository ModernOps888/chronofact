//! Explicit no-evidence state in claim verification and all-fields validation in memory_save.

use chronofact::grounding::claim::AtomicClaim;
use chronofact::mcp::protocol::JsonRpcRequest;
use chronofact::mcp::McpServer;
use chronofact::memory::MemoryEngine;
use chronofact::{ClaimCategory, FactVerifier, SourceChunk, VerificationStatus};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::tempdir;

fn claim(id: &str, s: &str) -> AtomicClaim {
    AtomicClaim {
        id: id.to_string(),
        statement: s.to_string(),
        category: ClaimCategory::FactualAssertion,
    }
}

async fn call_tool(server: &McpServer, name: &str, args: Value) -> (bool, String) {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({ "name": name, "arguments": args })),
    };
    let result = server.handle_request(req).await.unwrap().result.unwrap();
    let is_error = result["isError"].as_bool().unwrap_or(false);
    let text = result["content"][0]["text"].as_str().unwrap_or("").to_string();
    (is_error, text)
}

#[test]
fn test_no_sources_unchecked_claims_have_null_confidence_and_risk() {
    let verifier = FactVerifier::new();
    let claims = vec![
        claim("c1", "The billing service retries failed webhooks three times."),
        claim("c2", "Our staging cluster runs in eu-west-2."),
    ];
    let report = verifier.verify_claims(&claims, &[]);

    assert!(!report.grounding_sources_available);
    assert_eq!(report.unverified_count, 2);
    assert_eq!(report.checked_claims_count, 0);
    assert_eq!(report.hallucination_risk_index, None);
    for vc in &report.claims {
        assert_eq!(vc.status, VerificationStatus::Unverified);
        assert_eq!(vc.confidence_score, None, "unchecked claim must not carry a confidence");
    }

    let v = serde_json::to_value(&report).unwrap();
    assert_eq!(v["grounding_sources_available"], json!(false));
    assert!(v["hallucination_risk_index"].is_null());
    assert!(v["claims"][0]["confidence_score"].is_null());
}

#[test]
fn test_no_sources_invariant_hits_keep_real_scores() {
    let verifier = FactVerifier::new();
    let claims = vec![
        claim("bad", "Google DeepMind released Astra 6 as their new frontier flagship model."),
        claim("good", "OpenAI released GPT-6 Astra and Sol 6.1 in September 2026."),
        claim("unchecked", "Our staging cluster runs in eu-west-2."),
    ];
    let report = verifier.verify_claims(&claims, &[]);

    assert!(!report.grounding_sources_available);
    assert_eq!(report.claims[0].status, VerificationStatus::Contradicted);
    assert_eq!(report.claims[0].confidence_score, Some(0.01));
    assert_eq!(report.claims[1].status, VerificationStatus::Entailed);
    assert_eq!(report.claims[1].confidence_score, Some(0.99));
    assert_eq!(report.claims[2].confidence_score, None);
    assert_eq!(report.checked_claims_count, 2);
    // Risk over the 2 checked claims: 1 contradicted, 1 entailed -> linear 0.5 (> Noisy-OR floor 0.38).
    assert_eq!(report.hallucination_risk_index, Some(0.5));
}

#[test]
fn test_with_sources_flag_true_and_scores_present() {
    let verifier = FactVerifier::new();
    let sources = vec![SourceChunk {
        id: "S1".to_string(),
        url: "https://example.com".to_string(),
        title: "Docs".to_string(),
        content: "ChronoFact proxy server listens on port 3030 by default.".to_string(),
        integrity_hash: "h".to_string(),
        is_sanitized: true,
    }];
    let claims = vec![claim("c", "ChronoFact proxy server listens on port 3030 by default.")];
    let report = verifier.verify_claims(&claims, &sources);
    assert!(report.grounding_sources_available);
    assert_eq!(report.checked_claims_count, 1);
    assert!(report.claims[0].confidence_score.is_some());
    assert!(report.hallucination_risk_index.is_some());
}

#[tokio::test]
async fn test_mcp_verify_claims_reports_no_evidence_explicitly() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("v.db");
    let server = McpServer::new(Arc::new(MemoryEngine::open(db.to_str().unwrap()).unwrap()));

    let (is_error, text) = call_tool(
        &server,
        "chronofact_verify_claims",
        json!({ "response_text": "Our staging cluster runs in eu-west-2." }),
    )
    .await;
    assert!(!is_error, "{}", text);
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["grounding_sources_available"], json!(false));
    assert!(v["hallucination_risk_index"].is_null());
    assert_eq!(v["claims"][0]["status"], json!("Unverified"));
    assert!(v["claims"][0]["confidence_score"].is_null());
}

#[tokio::test]
async fn test_memory_save_reports_all_missing_fields_in_one_error() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("m.db");
    let server = McpServer::new(Arc::new(MemoryEngine::open(db.to_str().unwrap()).unwrap()));

    // Nothing supplied: every required field is listed in the single error.
    let (is_error, text) = call_tool(&server, "chronofact_memory_save", json!({})).await;
    assert!(is_error);
    assert!(
        text.contains("missing required field(s): project_id, entity_name, entity_type, definition"),
        "got: {}",
        text
    );

    // Partial: only the still-missing fields are listed; blank and non-string values are rejected.
    let (is_error, text) = call_tool(
        &server,
        "chronofact_memory_save",
        json!({ "project_id": "p1", "entity_name": "  ", "entity_type": 7 }),
    )
    .await;
    assert!(is_error);
    assert!(text.contains("missing required field(s): entity_name, definition"), "got: {}", text);
    assert!(text.contains("must be non-empty strings: entity_type"), "got: {}", text);
    assert!(!text.contains("missing required field(s): project_id"), "got: {}", text);

    // Complete: saves.
    let (is_error, text) = call_tool(
        &server,
        "chronofact_memory_save",
        json!({
            "project_id": "p1",
            "entity_name": "Database",
            "entity_type": "TECH_STACK",
            "definition": "PostgreSQL 17"
        }),
    )
    .await;
    assert!(!is_error, "{}", text);
    assert!(text.contains("\"status\":\"saved\""), "got: {}", text);
}

#[test]
fn test_memory_save_schema_declares_all_required_fields() {
    let required = chronofact::mcp::tools::required_fields("chronofact_memory_save");
    assert_eq!(required, vec!["project_id", "entity_name", "entity_type", "definition"]);
}

#[tokio::test]
async fn test_mcp_alias_flexibility_and_kls_lean4_invariants() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("alias.db");
    let server = McpServer::new(Arc::new(MemoryEngine::open(db.to_str().unwrap()).unwrap()));

    // 1. chronofact_verify_claims with 'claim' alias and 'grounding_sources' string array
    let (is_error, text) = call_tool(
        &server,
        "chronofact_verify_claims",
        json!({
            "claim": "The KLS conjecture bounds the Cheeger constant psi_n for isotropic log-concave distributions by a universal constant.",
            "grounding_sources": ["High-dimensional convex geometry KLS Cheeger-Poincaré spectral gap bound."]
        }),
    )
    .await;
    assert!(!is_error, "{}", text);
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["entailed_count"], json!(1));
    assert_eq!(v["claims"][0]["status"], json!("Entailed"));

    // 2. chronofact_memory_save with 'key' and 'value' aliases auto-fills project_id and entity_type
    let (is_error, text) = call_tool(
        &server,
        "chronofact_memory_save",
        json!({
            "key": "KLS_Spectral_Invariant",
            "value": "Enforce Cheeger-Poincaré spectral gap bound lambda_1 >= 1/(4*psi_n^2)."
        }),
    )
    .await;
    assert!(!is_error, "{}", text);
    assert!(text.contains("\"status\":\"saved\""), "got: {}", text);
}
