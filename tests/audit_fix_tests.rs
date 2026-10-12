//! Regression tests for the adversarial-audit fixes.
use chronofact::{ClaimExtractor, CodeInvariantChecker, FactVerifier, SourceChunk, TemporalScanner, VerificationStatus};

fn src(id: &str, content: &str) -> SourceChunk {
    SourceChunk {
        id: id.to_string(),
        title: "t".to_string(),
        url: "https://example.com".to_string(),
        content: content.to_string(),
        integrity_hash: String::new(),
        is_sanitized: true,
    }
}

fn verify(claim: &str, sources: &[SourceChunk]) -> VerificationStatus {
    let claims = ClaimExtractor::new().extract_claims(claim);
    assert!(!claims.is_empty(), "claim extractor dropped: {}", claim);
    FactVerifier::new().verify_claims(&claims, sources).claims[0].status.clone()
}

#[test]
fn negated_source_contradicts_affirmative_claim_without_support_verb() {
    let sources = [src("SRC-1", "The tokio runtime does not support the async-std executor interface.")];
    let st = verify("The tokio runtime has the async-std executor interface.", &sources);
    assert_eq!(st, VerificationStatus::Contradicted);

    let sources = [src("SRC-1", "The old routing layer was ripped out of the framework core in version 4.")];
    let st = verify("The framework core includes the old routing layer in version 4.", &sources);
    assert_eq!(st, VerificationStatus::Contradicted);
}

#[test]
fn matching_polarity_is_still_entailed() {
    let sources = [src("SRC-1", "The tokio runtime supports the async-std executor interface through a compatibility crate.")];
    let st = verify("The tokio runtime supports the async-std executor interface.", &sources);
    assert_eq!(st, VerificationStatus::Entailed);
}

#[test]
fn scanner_benign_numbers_and_words_score_low() {
    let sc = TemporalScanner::new();
    let r = sc.scan("What is the complexity of a binary search when the loss is 0.5 and we compute it now?");
    assert!(r.temporal_risk_score < 0.35, "score {} entities {:?} kw {:?}", r.temporal_risk_score, r.detected_entities, r.temporal_keywords);
    assert!(!r.requires_search);
}

#[test]
fn scanner_detects_newer_ecosystem_entities_and_entity_versions() {
    let sc = TemporalScanner::new();
    let r = sc.scan("How do I set up Zig with Svelte tooling?");
    assert!(r.detected_entities.iter().any(|e| e == "zig"));
    assert!(r.detected_entities.iter().any(|e| e == "svelte"));
    let r = sc.scan("breaking changes in tokio 1.38");
    assert!(r.detected_entities.iter().any(|e| e == "1.38"));
}

#[test]
fn code_invariants_exec_and_function_constructor() {
    let chk = CodeInvariantChecker::new();
    let ok = chk.audit_code("src/util.js", "const m = re.exec(text);\n", None);
    assert_eq!(ok.violations_count, 0, "regex .exec( is not code execution");

    let bad = chk.audit_code("src/util.js", "const f = new Function(userInput);\n", None);
    assert!(bad.violations.iter().any(|v| v.rule_id == "INV-SEC-EVAL-EXEC"));
    let bad2 = chk.audit_code("src/util.py", "os.system(cmd)\n", None);
    assert!(bad2.violations.iter().any(|v| v.rule_id == "INV-SEC-EVAL-EXEC"));
}

#[test]
fn code_invariants_frontend_path_is_segment_based_and_block_comments_skipped() {
    let chk = CodeInvariantChecker::new();
    let line = "import { PrismaClient } from \"@prisma/client\";\n";
    let backend = chk.audit_code("src/guide/quiz.ts", line, None);
    assert!(!backend.violations.iter().any(|v| v.rule_id == "INV-ARCH-DAL-LEAK"));
    let ui = chk.audit_code("src/components/Table.tsx", line, None);
    assert!(ui.violations.iter().any(|v| v.rule_id == "INV-ARCH-DAL-LEAK"));

    let doc = chk.audit_code("src/a.js", "/*\n * eval(x) is dangerous\n */\n", None);
    assert_eq!(doc.violations_count, 0);
}

#[test]
fn attestation_is_unsigned_without_key_and_verifies_with_key() {
    use chronofact::grounding::attestation::{AttestationEngine, UNSIGNED_MARKER};
    let chk = CodeInvariantChecker::new();
    let report = chk.audit_code("a.rs", "fn main() {}\n", None);
    let unsigned = AttestationEngine::create_attestation(&report, "p", "fn main() {}", "2026-10", Some(""));
    // Empty explicit key and no env key => unsigned (env var is not set in CI)
    if std::env::var("CHRONOFACT_ATTESTATION_KEY").is_err() {
        assert_eq!(unsigned.signature, UNSIGNED_MARKER);
        assert!(!AttestationEngine::verify_signature(&unsigned, Some("")));
    }
    let signed = AttestationEngine::create_attestation(&report, "p", "fn main() {}", "2026-10", Some("k1"));
    assert!(AttestationEngine::verify_signature(&signed, Some("k1")));
    assert!(!AttestationEngine::verify_signature(&signed, Some("k2")));
}
