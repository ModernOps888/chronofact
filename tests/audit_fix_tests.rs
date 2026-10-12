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

#[test]
fn registry_candidates_are_curated_not_every_word() {
    use chronofact::{PackageEcosystem, PackageRegistryClient};
    let c = PackageRegistryClient::candidates_from_query("What is the latest version of tokio and react?");
    let names: Vec<&str> = c.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["tokio", "react"], "only curated packages, never English words");
    assert_eq!(c[0].1, PackageEcosystem::Cargo);
    assert_eq!(c[1].1, PackageEcosystem::Npm);
    // alias mapping + dedupe + cap
    let c = PackageRegistryClient::candidates_from_query("tailwind tailwindcss vue svelte nuxt astro hono");
    assert_eq!(c.len(), 4);
    assert_eq!(c[0].0, "tailwindcss");
    // no candidates for plain prose
    assert!(PackageRegistryClient::candidates_from_query("what is the latest version of the thing").is_empty());
}


// ---------------------------------------------------------------------------
// Fix batch 3: clause-scoped verifier, scanner entities, invariant patterns.
// Cases include controls (must NOT fire) so the fixes cannot pass by over-matching.
// ---------------------------------------------------------------------------

#[test]
fn verifier_concession_about_something_else_does_not_contradict_or_block_entailment() {
    let s = [src("S", "Deno 2 is compatible with most npm packages, although some native addons are incompatible.")];
    assert_eq!(verify("Deno 2 is compatible with most npm packages.", &s), VerificationStatus::Entailed);
    let s = [src("S", "Fastify 5 is compatible with Node 20 but incompatible with Node 14.")];
    assert_eq!(verify("Fastify 5 is compatible with Node 20.", &s), VerificationStatus::Entailed);
    let s = [src("S", "Bun 1.3 supports Windows natively, although some APIs are unsupported.")];
    assert_eq!(verify("Bun 1.3 supports Windows natively.", &s), VerificationStatus::Entailed);
}

#[test]
fn verifier_still_contradicts_when_negation_is_about_the_claim() {
    let s = [src("S", "Rust 2024 edition is unavailable on stable.")];
    assert_eq!(verify("Rust 2024 edition is available.", &s), VerificationStatus::Contradicted);
    let s = [src("S", "Next.js 16 removed the Pages Router entirely.")];
    assert_eq!(verify("Next.js 16 supports the Pages Router.", &s), VerificationStatus::Contradicted);
    let s = [src("S", "Svelte 5 is not stable.")];
    assert_eq!(verify("Svelte 5 is stable.", &s), VerificationStatus::Contradicted);
    // a claim that is itself negated must not be contradicted by a matching negated source
    let s = [src("S", "The legacy API is unsupported in v3.")];
    assert_ne!(verify("The legacy API is not supported in v3.", &s), VerificationStatus::Contradicted);
}

#[test]
fn scanner_new_entities_flag_and_plain_go_does_not() {
    let sc = TemporalScanner::new();
    for q in [
        "What's new in Bun 1.3?",
        "Breaking changes in Fastify 5?",
        "Migrating a LangChain app to LangGraph",
        "Go 1.25 release notes",
        "Latest Hono middleware for JWT auth",
    ] {
        assert!(sc.scan(q).requires_search, "should require grounding: {q}");
    }
    for q in [
        "How do I go about learning recursion?",
        "Let's go through the binary search algorithm step by step.",
        "Can you go over how hash maps handle collisions?",
        "Explain the difference between a mutex and a semaphore.",
    ] {
        assert!(!sc.scan(q).requires_search, "must not require grounding: {q}");
    }
}

#[test]
fn invariants_catch_python_sqli_shell_true_and_string_timers_without_false_positives() {
    let c = CodeInvariantChecker::new();
    let hit = |path: &str, code: &str| c.audit_code(path, code, None).violations_count > 0;
    for (p, code) in [
        ("a.py", "cursor.execute(f\"SELECT * FROM users WHERE id = {user_id}\")"),
        ("a.py", "cur.execute(\"DELETE FROM t WHERE id = {}\".format(i))"),
        ("a.py", "cur.execute(\"SELECT * FROM t WHERE n = '%s'\" % name)"),
        ("a.py", "subprocess.run(cmd, shell=True)"),
        ("a.py", "subprocess.Popen(args, shell=True)"),
        ("a.js", "setTimeout(\"alert(1)\", 0)"),
        ("a.js", "setInterval(\"tick()\", 1000)"),
    ] {
        assert!(hit(p, code), "should flag: {code}");
    }
    for (p, code) in [
        ("a.py", "cur.execute(\"SELECT * FROM t WHERE id = %s\", (uid,))"),
        ("a.py", "cursor.execute(\"SELECT * FROM users WHERE id = ?\", (user_id,))"),
        ("a.py", "subprocess.run([\"ls\", \"-l\"], shell=False)"),
        ("a.py", "subprocess.run(cmd)"),
        ("a.py", "print(f\"Delete {n} files\")"),
        ("a.js", "setTimeout(() => run(), 100)"),
        ("a.js", "setTimeout(fn, 100)"),
    ] {
        assert!(!hit(p, code), "must not flag: {code}");
    }
}


#[test]
fn mcp_stdout_carries_only_json_rpc_even_with_verbose_logging() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_chronofact"))
        .arg("mcp")
        .env("RUST_LOG", "debug")
        // Force the web search to fail offline so a WARN log is emitted (reproduces the stdout-pollution bug).
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .env("CHRONOFACT_DB", std::env::temp_dir().join("cf_stdout_test.db"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn chronofact mcp");
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2024-11-05","capabilities":{{}},"clientInfo":{{"name":"t","version":"1"}}}}}}"#).unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#).unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"chronofact_ground_query","arguments":{{"query":"latest version of foobar library","max_results":3}}}}}}"#).unwrap();
    drop(stdin);
    let reader = BufReader::new(child.stdout.take().unwrap());
    let mut n = 0;
    for line in reader.lines() {
        let line = line.unwrap();
        if line.trim().is_empty() { continue; }
        assert!(serde_json::from_str::<serde_json::Value>(&line).is_ok(), "non-JSON line on MCP stdout: {line:?}");
        n += 1;
    }
    let _ = child.wait();
    assert!(n >= 2, "expected initialize + ground_query responses, got {n}");
}
