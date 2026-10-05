use chronofact::{ClaimCategory, ClaimExtractor, FactVerifier, SourceChunk, VerificationStatus};

#[test]
fn test_claim_extractor_filters_filler() {
    let extractor = ClaimExtractor::new();
    let text = "Hello there! I can help you with that. Next.js 16 deprecated server actions with the old syntax. The function configureRoutes now takes an options object.";
    
    let claims = extractor.extract_claims(text);
    // Should filter "Hello there!" and "I can help you with that"
    assert_eq!(claims.len(), 2);
    assert_eq!(claims[0].category, ClaimCategory::VersionCompatibility);
    assert_eq!(claims[1].category, ClaimCategory::TechnicalApi);
}

#[test]
fn test_fact_verifier_entailment_and_contradiction() {
    let extractor = ClaimExtractor::new();
    let verifier = FactVerifier::new();

    let sources = vec![
        SourceChunk {
            id: "SRC-1".to_string(),
            title: "Next.js 16 Migration Guide".to_string(),
            url: "https://nextjs.org/docs".to_string(),
            content: "In version 16, server actions with the old syntax are deprecated. You must use the new actions module.".to_string(),
            integrity_hash: "hash123".to_string(),
            is_sanitized: true,
        },
        SourceChunk {
            id: "SRC-2".to_string(),
            title: "Legacy API Docs".to_string(),
            url: "https://nextjs.org/legacy".to_string(),
            content: "The old syntax is not supported in Next.js 16 and has been removed.".to_string(),
            integrity_hash: "hash456".to_string(),
            is_sanitized: true,
        },
    ];

    // True statement matching evidence
    let true_text = "Next.js 16 deprecated server actions with the old syntax.";
    let claims = extractor.extract_claims(true_text);
    let report = verifier.verify_claims(&claims, &sources);

    assert_eq!(report.entailed_count, 1);
    assert_eq!(report.contradicted_count, 0);
    assert_eq!(report.claims[0].status, VerificationStatus::Entailed);
    assert!(report.claims[0].matched_source_ids.contains(&"SRC-1".to_string()));

    // Contradictory statement asserting support when sources state removal/unsupported
    let false_text = "Next.js 16 supports the old syntax seamlessly with full backwards compatibility.";
    let false_claims = extractor.extract_claims(false_text);
    let false_report = verifier.verify_claims(&false_claims, &sources);

    assert_eq!(false_report.contradicted_count, 1);
    assert_eq!(false_report.claims[0].status, VerificationStatus::Contradicted);
    assert!(false_report.hallucination_risk_index > 0.5);
}

#[test]
fn test_astra_openai_vs_google_contradiction() {
    let extractor = ClaimExtractor::new();
    let verifier = FactVerifier::new();

    // 1. False claim: Astra 6 is from Google DeepMind
    let false_text = "Google DeepMind released Astra 6 as their new frontier flagship model.";
    let claims = extractor.extract_claims(false_text);
    let report = verifier.verify_claims(&claims, &[]);

    assert_eq!(report.contradicted_count, 1);
    assert_eq!(report.claims[0].status, VerificationStatus::Contradicted);
    assert!(report.claims[0].rationale.contains("OpenAI, NOT Google"));

    // 2. True claim: Astra 6 is from OpenAI
    let true_text = "OpenAI released GPT-6 Astra and Sol 6.1 in September 2026.";
    let true_claims = extractor.extract_claims(true_text);
    let true_report = verifier.verify_claims(&true_claims, &[]);

    assert_eq!(true_report.entailed_count, 1);
    assert_eq!(true_report.claims[0].status, VerificationStatus::Entailed);
    assert_eq!(true_report.contradicted_count, 0);
}

