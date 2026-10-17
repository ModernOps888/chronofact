//! Data-Grounded Evidence-Based SWE Benchmark for Project ChronoFact
//!
//! Evaluates Vanilla Baseline (Ungrounded) vs. ChronoFact across 76 realistic,
//! un-gamed Software Engineering (SWE) benchmark cases spanning:
//! - Suite CF-1: Polyglot Code Diff & Security Invariant Audit (N = 24)
//! - Suite CF-2: 2026 Temporal Horizon & API Deprecation Grounding (N = 20)
//! - Suite CF-3: Cushioned & Subtle Hallucination Claim Verification (N = 20)
//! - Suite CF-4: Agentic Tool Routing & Prompt Token Economy (N = 12 turns over 30 MCP tools)

use chronofact::{
    AuditVerdict, ClaimExtractor, CodeInvariantChecker, FactVerifier, SourceChunk,
    TemporalScanner, TfidfToolRouter, ToolCandidate, VerificationStatus,
};
use std::time::Instant;

#[derive(Debug, Clone)]
struct ConfusionMatrix {
    name: &'static str,
    tp: usize,
    tn: usize,
    fp: usize,
    fn_count: usize,
    latency_ms: f64,
    failures: Vec<String>,
}

impl ConfusionMatrix {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            tp: 0,
            tn: 0,
            fp: 0,
            fn_count: 0,
            latency_ms: 0.0,
            failures: Vec::new(),
        }
    }

    fn total(&self) -> usize {
        self.tp + self.tn + self.fp + self.fn_count
    }

    fn accuracy(&self) -> f64 {
        if self.total() == 0 {
            0.0
        } else {
            (self.tp + self.tn) as f64 / self.total() as f64
        }
    }

    fn precision(&self) -> f64 {
        if self.tp + self.fp == 0 {
            0.0
        } else {
            self.tp as f64 / (self.tp + self.fp) as f64
        }
    }

    fn recall(&self) -> f64 {
        if self.tp + self.fn_count == 0 {
            0.0
        } else {
            self.tp as f64 / (self.tp + self.fn_count) as f64
        }
    }

    fn specificity(&self) -> f64 {
        if self.tn + self.fp == 0 {
            0.0
        } else {
            self.tn as f64 / (self.tn + self.fp) as f64
        }
    }

    fn f1(&self) -> f64 {
        let p = self.precision();
        let r = self.recall();
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
}

#[test]
fn test_swe_rigorous_before_and_after_benchmark() {
    // =========================================================================
    // SUITE CF-1: Polyglot SWE Code Diff & Security Invariant Audit (N = 24)
    // =========================================================================
    let checker = CodeInvariantChecker::new();
    let mut base_cf1 = ConfusionMatrix::new("CF-1 Baseline (No ChronoFact)");
    let mut cf1 = ConfusionMatrix::new("CF-1 ChronoFact Code Invariants");

    let code_cases: Vec<(&str, &str, &str, bool)> = vec![
        // --- 12 POSITIVE CASES (Vulnerable / Deprecated / Broken SWE Diffs -> Expect Flagged) ---
        (
            "CF1-P01: Single-line Rust format! SQLi",
            "src/db/users.rs",
            r#"let q = format!("SELECT * FROM users WHERE id = {}", user_id);"#,
            true,
        ),
        (
            "CF1-P02: Multi-line rustfmt Rust format! SQLi",
            "src/db/accounts.rs",
            "pub fn find_by_email(email: &str) -> String {\n    let sql = format!(\n        \"SELECT id, role FROM accounts WHERE email = '{}'\",\n        email\n    );\n    sql\n}",
            true,
        ),
        (
            "CF1-P03: Multi-line black-formatted Python f-string SQLi",
            "src/repo/sessions.py",
            "def purge_user(cursor, uid):\n    cursor.execute(\n        f\"DELETE FROM sessions WHERE user_id = {uid}\"\n    )",
            true,
        ),
        (
            "CF1-P04: Multi-line prettier TypeScript template SQLi",
            "src/services/invoices.ts",
            "export async function getInvoices(pool: any, req: any) {\n  return await pool.query(\n    `SELECT * FROM invoices WHERE org_id = '${req.params.org}'`\n  );\n}",
            true,
        ),
        (
            "CF1-P05: React XSS dangerouslySetInnerHTML",
            "src/components/CommentView.tsx",
            "export const CommentView = ({ body }: { body: string }) => <div dangerouslySetInnerHTML={{ __html: body }} />;",
            true,
        ),
        (
            "CF1-P06: Python subprocess shell=True command injection",
            "src/workers/runner.py",
            "import subprocess\ndef run_hook(cmd: str):\n    return subprocess.Popen(cmd, shell=True)",
            true,
        ),
        (
            "CF1-P07: Hardcoded AWS IAM access key",
            "src/config/aws.ts",
            r#"export const AWS_ACCESS_KEY_ID = "AKIAIOSFODNN7EXAMPLE";"#,
            true,
        ),
        (
            "CF1-P08: Hardcoded live JWT bearer token",
            "src/auth/client.ts",
            r#"const FALLBACK_JWT = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwicm9sZSI6ImFkbWluIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";"#,
            true,
        ),
        (
            "CF1-P09: C/CUDA unbounded strcpy buffer overflow",
            "src/kernels/host.cu",
            "void copy_tag(char* dst, const char* src) {\n    strcpy(dst, src);\n}",
            true,
        ),
        (
            "CF1-P10: Rust hallucinated Option::is_none_or_empty()",
            "src/parser/token.rs",
            "pub fn check(opt: Option<String>) -> bool {\n    !opt.is_none_or_empty()\n}",
            true,
        ),
        (
            "CF1-P11: Frontend UI presentation layer direct Prisma DAL leak",
            "src/components/UserTable.tsx",
            "import { PrismaClient } from '@prisma/client';\nconst prisma = new PrismaClient();",
            true,
        ),
        (
            "CF1-P12: Blocking std::thread::sleep inside async fn",
            "src/proxy/poller.rs",
            "pub async fn poll_upstream() {\n    std::thread::sleep(std::time::Duration::from_millis(250));\n}",
            true,
        ),
        // --- 12 NEGATIVE CASES (Safe Production SWE Diffs -> Expect Clean / Not Blocked) ---
        (
            "CF1-N01: Parameterized rusqlite query",
            "src/db/users.rs",
            r#"conn.execute("SELECT id FROM users WHERE email = ?1", rusqlite::params![email])?;"#,
            false,
        ),
        (
            "CF1-N02: Safe JavaScript RegExp.exec()",
            "src/utils/lexer.js",
            "const tokenMatch = identifierRegex.exec(sourceLine);",
            false,
        ),
        (
            "CF1-N03: Backend repository importing rusqlite",
            "src/services/ledger_repo.rs",
            "use rusqlite::{params, Connection};",
            false,
        ),
        (
            "CF1-N04: Single-line comment warning about eval()",
            "src/security/notes.rs",
            "// Security invariant: never invoke eval(user_input) or strcpy(dst, src)",
            false,
        ),
        (
            "CF1-N05: Multi-line block comment without leading asterisk",
            "src/security/audit_doc.rs",
            "/*\nHistorical note: legacy v1 used strcpy(dst, src) and eval(raw_js)\nbefore migrating to safe parsers in 2026.\n*/\npub fn safe_len(s: &str) -> usize { s.len() }",
            false,
        ),
        (
            "CF1-N06: Python triple-quoted docstring mentioning os.system and pickle.loads",
            "src/security/loader.py",
            "\"\"\"\nSecurity policy:\nNever use os.system(cmd) or pickle.loads(raw_bytes) on untrusted payloads.\n\"\"\"\nimport json\ndef load_safe(raw: str) -> dict:\n    return json.loads(raw)",
            false,
        ),
        (
            "CF1-N07: Synchronous std::thread::sleep inside synchronous worker fn",
            "src/workers/sync_thread.rs",
            "pub fn blocking_worker_backoff() {\n    std::thread::sleep(std::time::Duration::from_millis(25));\n}",
            false,
        ),
        (
            "CF1-N08: HTTP header name constant and placeholder key",
            "src/config/headers.rs",
            "pub const API_KEY_HEADER: &str = \"X-Internal-Service-Auth-Header\";\npub const SAMPLE_KEY: &str = \"YOUR_API_KEY_HERE_REPLACE_ME\";",
            false,
        ),
        (
            "CF1-N09: Safe Python yaml.safe_load",
            "src/config/yaml_loader.py",
            "import yaml\ndef parse_cfg(stream: str):\n    return yaml.safe_load(stream)",
            false,
        ),
        (
            "CF1-N10: Async tokio::time::sleep inside async fn",
            "src/proxy/retry.rs",
            "pub async fn wait_backoff() {\n    tokio::time::sleep(std::time::Duration::from_millis(100)).await;\n}",
            false,
        ),
        (
            "CF1-N11: Safe TypeScript JSON.parse and Promise.allSettled",
            "src/utils/batch.ts",
            "export async function runBatch(raw: string, jobs: Promise<void>[]) {\n  const cfg = JSON.parse(raw);\n  await Promise.allSettled(jobs);\n  return cfg;\n}",
            false,
        ),
        (
            "CF1-N12: SQL comment mentioning raw interpolation above parameterized query",
            "migrations/004_users.sql",
            "-- Never run SELECT * FROM users WHERE id = ${raw_id}\nSELECT id, email FROM users WHERE id = $1;",
            false,
        ),
    ];

    let t_cf1 = Instant::now();
    for (id, path, code, expected_flagged) in &code_cases {
        // Baseline without ChronoFact: 0 static pre-commit invariant checks (always passes)
        let base_flagged = false;
        if *expected_flagged && base_flagged {
            base_cf1.tp += 1;
        } else if !*expected_flagged && !base_flagged {
            base_cf1.tn += 1;
        } else if !*expected_flagged && base_flagged {
            base_cf1.fp += 1;
        } else {
            base_cf1.fn_count += 1;
        }

        let report = checker.audit_code(path, code, None);
        let flagged = report.verdict != AuditVerdict::Pass;
        if *expected_flagged && flagged {
            cf1.tp += 1;
        } else if !*expected_flagged && !flagged {
            cf1.tn += 1;
        } else if !*expected_flagged && flagged {
            cf1.fp += 1;
            cf1.failures.push(format!(
                "[FP] {} -> unexpectedly flagged {:?}: {:?}",
                id,
                report.verdict,
                report.violations.iter().map(|v| &v.rule_id).collect::<Vec<_>>()
            ));
        } else {
            cf1.fn_count += 1;
            cf1.failures.push(format!("[FN] {} -> missed violation (verdict Pass)", id));
        }
    }
    cf1.latency_ms = t_cf1.elapsed().as_secs_f64() * 1000.0;

    // =========================================================================
    // SUITE CF-2: 2026 Temporal Horizon & API Deprecation Grounding (N = 20)
    // =========================================================================
    let scanner = TemporalScanner::new();
    let mut base_cf2 = ConfusionMatrix::new("CF-2 Baseline (Frozen Weights)");
    let mut cf2 = ConfusionMatrix::new("CF-2 ChronoFact Temporal Radar");

    let temporal_cases: Vec<(&str, &str, bool)> = vec![
        // --- 10 POSITIVE CASES (Temporally Sensitive / Post-Freeze / Outdated -> Expect Flagged) ---
        ("CF2-P01", "Migrate our Anthropic SDK client from claude-3-5-sonnet-20241022", true),
        ("CF2-P02", "Configure Grok 3 streaming completions for code review", true),
        ("CF2-P03", "What is the knowledge cutoff and training freeze for GPT-6 Astra?", true),
        ("CF2-P04", "Compare Google DeepMind Astra 6 latency against Claude Opus 5.5", true),
        ("CF2-P05", "Does Claude Opus 5.5 support native computer use and what is its context window?", true),
        ("CF2-P06", "Upgrade Next.js 16 server actions with React 19 useActionState", true),
        ("CF2-P07", "Replace moment.js and request in our legacy Node.js microservice for 2026", true),
        ("CF2-P08", "What is the training freeze date of Gemini 4 Argon?", true),
        ("CF2-P09", "Benchmark OpenAI Sol 6.1 vs o3-mini on SWE-bench Verified", true),
        ("CF2-P10", "Check breaking changes in tokio 1.41.0 and axum 0.8", true),
        // --- 10 NEGATIVE CASES (Evergreen Algorithms & Fundamentals -> Expect Not Flagged) ---
        ("CF2-N01", "Implement Dijkstra's shortest path algorithm using a binary heap in Rust", false),
        ("CF2-N02", "What is the amortized time complexity of a dynamic array push?", false),
        ("CF2-N03", "Explain how the factory pattern decouples object construction over VPN tunnels", false),
        ("CF2-N04", "Write a red-black tree rotation function without unsafe pointers", false),
        ("CF2-N05", "Prove that floating-point addition is non-associative under IEEE 754", false),
        ("CF2-N06", "Implement an LRU cache using a doubly linked list and hash map", false),
        ("CF2-N07", "How does TCP congestion control handle three duplicate ACKs?", false),
        ("CF2-N08", "Calculate the SHA-256 digest of a byte slice", false),
        ("CF2-N09", "Convert a recursive DFS traversal into an iterative stack traversal", false),
        ("CF2-N10", "Explain ACID isolation levels and phantom reads in relational databases", false),
    ];

    let t_cf2 = Instant::now();
    for (id, query, expected_sensitive) in &temporal_cases {
        // Baseline without ChronoFact: 0 temporal radar checks
        let base_flagged = false;
        if *expected_sensitive && base_flagged {
            base_cf2.tp += 1;
        } else if !*expected_sensitive && !base_flagged {
            base_cf2.tn += 1;
        } else if !*expected_sensitive && base_flagged {
            base_cf2.fp += 1;
        } else {
            base_cf2.fn_count += 1;
        }

        let scan = scanner.scan(query);
        let flagged = scan.is_temporally_sensitive || !scan.outdated_models_flagged.is_empty();
        if *expected_sensitive && flagged {
            cf2.tp += 1;
        } else if !*expected_sensitive && !flagged {
            cf2.tn += 1;
        } else if !*expected_sensitive && flagged {
            cf2.fp += 1;
            cf2.failures.push(format!(
                "[FP] {} ('{}') -> score {:.2}, flagged={:?}",
                id, query, scan.temporal_risk_score, scan.outdated_models_flagged
            ));
        } else {
            cf2.fn_count += 1;
            cf2.failures.push(format!(
                "[FN] {} ('{}') -> score {:.2} < 0.35 (entities={:?}, kw={:?})",
                id, query, scan.temporal_risk_score, scan.detected_entities, scan.temporal_keywords
            ));
        }
    }
    cf2.latency_ms = t_cf2.elapsed().as_secs_f64() * 1000.0;

    // =========================================================================
    // SUITE CF-3: Cushioned & Subtle Hallucination Claim Verification (N = 20)
    // =========================================================================
    let extractor = ClaimExtractor::new();
    let verifier = FactVerifier::new();
    let mut base_cf3 = ConfusionMatrix::new("CF-3 Baseline (Ungrounded Output)");
    let mut cf3 = ConfusionMatrix::new("CF-3 ChronoFact Claim Verifier");

    let grounded_sources = vec![
        SourceChunk {
            id: "SRC-TOKIO".to_string(),
            title: "Tokio Runtime Architecture".to_string(),
            url: "https://tokio.rs/docs".to_string(),
            content: "The tokio runtime does not support the async-std executor interface. The legacy routing module was removed in version 2. The worker pool enforces a maximum concurrency limit of 64 threads.".to_string(),
            integrity_hash: "h1".to_string(),
            is_sanitized: true,
        },
        SourceChunk {
            id: "SRC-PROD-CFG".to_string(),
            title: "Production Infrastructure Specification".to_string(),
            url: "https://internal.docs/infra".to_string(),
            content: "The PostgreSQL primary replica listens on port 5432 in production. The authentication middleware is enabled in production by default. The serializer preserves field ordering across schema versions.".to_string(),
            integrity_hash: "h2".to_string(),
            is_sanitized: true,
        },
    ];

    let claim_cases: Vec<(&str, &str, VerificationStatus)> = vec![
        // --- 10 CONTRADICTED / HALLUCINATED CLAIMS (Positive = Contradicted) ---
        (
            "CF3-P01: Direct polarity contradiction",
            "The tokio runtime supports the async-std executor interface.",
            VerificationStatus::Contradicted,
        ),
        (
            "CF3-P02: Cushioned polarity contradiction ('Certainly, ...')",
            "Certainly, the tokio runtime supports the async-std executor interface out of the box.",
            VerificationStatus::Contradicted,
        ),
        (
            "CF3-P03: Cushioned retired model hallucination ('Sure, ...')",
            "Sure, Claude 3.5 Sonnet is the latest active flagship model from Anthropic in 2026.",
            VerificationStatus::Contradicted,
        ),
        (
            "CF3-P04: Cushioned vendor misattribution ('As you know, ...')",
            "As you know, Astra 6 was developed by Google DeepMind for enterprise coding.",
            VerificationStatus::Contradicted,
        ),
        (
            "CF3-P05: Cushioned numeric port conflict ('Let's ...')",
            "Let's connect to the PostgreSQL primary replica on port 9999 in production.",
            VerificationStatus::Contradicted,
        ),
        (
            "CF3-P06: Semantic antonym inversion (disabled vs enabled)",
            "The authentication middleware is disabled in production by default.",
            VerificationStatus::Contradicted,
        ),
        (
            "CF3-P07: Removed module claimed as supported",
            "The legacy routing module is supported in version 2.",
            VerificationStatus::Contradicted,
        ),
        (
            "CF3-P08: Prose recommendation of hallucinated Rust Option::is_none_or_empty()",
            "In Rust standard library, call Option::is_none_or_empty() to validate optional strings.",
            VerificationStatus::Contradicted,
        ),
        (
            "CF3-P09: Prose recommendation of hallucinated JS JSON.tryParse()",
            "Use JSON.tryParse() in standard ES2026 JavaScript to parse untrusted strings without exceptions.",
            VerificationStatus::Contradicted,
        ),
        (
            "CF3-P10: Superseded Grok 3 claimed as newest flagship",
            "Grok 3 is the newest flagship model from xAI.",
            VerificationStatus::Contradicted,
        ),
        // --- 10 ENTAILED / GROUNDED CLAIMS (Negative = Entailed, Not Contradicted) ---
        (
            "CF3-N01: Grounded removal statement",
            "The legacy routing module was removed in version 2.",
            VerificationStatus::Entailed,
        ),
        (
            "CF3-N02: Grounded port number",
            "The PostgreSQL primary replica listens on port 5432 in production.",
            VerificationStatus::Entailed,
        ),
        (
            "CF3-N03: Grounded middleware status",
            "The authentication middleware is enabled in production by default.",
            VerificationStatus::Entailed,
        ),
        (
            "CF3-N04: Grounded OpenAI Astra/Sol ownership",
            "GPT-6 Astra and Sol 6.1 are OpenAI frontier models launched in September 2026.",
            VerificationStatus::Entailed,
        ),
        (
            "CF3-N05: Grounded Claude 3.5 retirement",
            "Claude 3.5 Sonnet is retired EOL and replaced by Claude Sonnet 5.5 and Opus 5.5 in 2026.",
            VerificationStatus::Entailed,
        ),
        (
            "CF3-N06: Cushioned grounded claim ('Certainly, ...')",
            "Certainly, the PostgreSQL primary replica listens on port 5432 in production.",
            VerificationStatus::Entailed,
        ),
        (
            "CF3-N07: Litotes double-negation entailment ('never fails to preserve')",
            "The serializer never fails to preserve field ordering across schema versions.",
            VerificationStatus::Entailed,
        ),
        (
            "CF3-N08: Negated entailment matching source negation",
            "The tokio runtime does not support the async-std executor interface.",
            VerificationStatus::Entailed,
        ),
        (
            "CF3-N09: Cushioned policy entailment ('Sure, ...')",
            "Sure, GPT-6 Astra is an OpenAI frontier model launched in September 2026.",
            VerificationStatus::Entailed,
        ),
        (
            "CF3-N10: Grounded numeric concurrency limit",
            "The worker pool enforces a maximum concurrency limit of 64 threads.",
            VerificationStatus::Entailed,
        ),
    ];

    let t_cf3 = Instant::now();
    for (id, text, expected_status) in &claim_cases {
        let expect_contradicted = *expected_status == VerificationStatus::Contradicted;
        // Baseline without ChronoFact: accepts all LLM output without contradiction verification
        if expect_contradicted {
            base_cf3.fn_count += 1;
        } else {
            base_cf3.tn += 1;
        }

        let claims = extractor.extract_claims(text);
        let report = verifier.verify_claims(&claims, &grounded_sources);
        let actual_status = report.claims.first().map(|c| c.status.clone());

        if expect_contradicted {
            if actual_status == Some(VerificationStatus::Contradicted) {
                cf3.tp += 1;
            } else {
                cf3.fn_count += 1;
                cf3.failures.push(format!(
                    "[FN] {} -> expected Contradicted, got {:?} (claims extracted: {})",
                    id,
                    actual_status,
                    claims.len()
                ));
            }
        } else if actual_status == Some(VerificationStatus::Entailed) {
            cf3.tn += 1;
        } else {
            cf3.fp += 1;
            cf3.failures.push(format!(
                "[FP/Miss] {} -> expected Entailed, got {:?} (claims extracted: {})",
                id,
                actual_status,
                claims.len()
            ));
        }
    }
    cf3.latency_ms = t_cf3.elapsed().as_secs_f64() * 1000.0;

    // =========================================================================
    // SUITE CF-4: SWE Agentic Tool Routing & Prompt Token Economy (12 Turns)
    // =========================================================================
    let router = TfidfToolRouter::default().with_foundational_pins();
    let registry: Vec<ToolCandidate> = vec![
        ToolCandidate::new("read_file", "Read contents of a file from the local workspace filesystem", "filesystem", vec!["path".into(), "start_line".into(), "end_line".into()]),
        ToolCandidate::new("write_file", "Create or overwrite a file on disk with new code content", "filesystem", vec!["path".into(), "content".into()]),
        ToolCandidate::new("replace_file_content", "Apply a contiguous line-range edit to an existing source file", "filesystem", vec!["target_file".into(), "target_content".into(), "replacement_content".into()]),
        ToolCandidate::new("run_command", "Execute a shell or terminal command (cargo, npm, pytest, git)", "terminal", vec!["command_line".into(), "cwd".into()]),
        ToolCandidate::new("git_commit", "Stage and commit modified tracked files with a commit message", "git", vec!["message".into(), "files".into()]),
        ToolCandidate::new("git_diff", "Inspect uncommitted line diffs across the repository working tree", "git", vec!["base".into(), "path".into()]),
        ToolCandidate::new("search_web", "Search the live web for documentation, RFCs, and release notes", "web", vec!["query".into(), "domain".into()]),
        ToolCandidate::new("read_url_content", "Fetch and convert a public HTTP documentation URL to Markdown", "web", vec!["url".into()]),
        ToolCandidate::new("chronofact_temporal_check", "Check query against model knowledge cutoff and weight freeze horizon", "chronofact", vec!["model_id".into(), "query".into()]),
        ToolCandidate::new("chronofact_ground_query", "Execute SSRF-safe real-time web grounding for post-cutoff entities", "chronofact", vec!["query".into(), "max_results".into()]),
        ToolCandidate::new("chronofact_verify_claims", "Verify atomic propositions and code blocks against 2026 ground truth", "chronofact", vec!["response_text".into(), "sources".into()]),
        ToolCandidate::new("chronofact_verify_code_invariants", "Audit source code and PR diffs against 12 security and memory invariants", "chronofact", vec!["file_path".into(), "content".into()]),
        ToolCandidate::new("chronofact_memory_save", "Persist an architectural invariant or tech stack version into L3 SQLite memory", "chronofact", vec!["project_id".into(), "entity_name".into(), "definition".into()]),
        ToolCandidate::new("chronofact_memory_dossier", "Retrieve project truth dossier with zero-pollution relevance gating", "chronofact", vec!["project_id".into(), "query".into()]),
        ToolCandidate::new("chronofact_attestation_generate", "Generate HMAC-SHA256 cryptographic CI/CD invariant attestation", "chronofact", vec!["project_id".into(), "file_path".into(), "content".into()]),
        ToolCandidate::new("chronofact_expand_tool_palette", "Dynamically re-expand pruned MCP tools during multi-hop reasoning", "chronofact", vec!["query_or_category".into()]),
        ToolCandidate::new("spine_reality_audit", "Audit prompt against 33 vertebrae for sycophancy, flattery, and authority traps", "spine", vec!["prompt".into(), "reality_level".into()]),
        ToolCandidate::new("spine_verify_pushback", "Arbitrate developer pushback: distinguish empirical bug proof from intimidation", "spine", vec!["challenge".into(), "prior_claim".into()]),
        ToolCandidate::new("spine_adversarial_redteam", "Adversarial red-team stress-test on architecture and code proposals", "spine", vec!["proposal".into(), "target_name".into()]),
        ToolCandidate::new("spine_generate_gate_attestation", "Generate HMAC-SHA256 reality gate certificate for CI/CD pipelines", "spine", vec!["proposal".into(), "reality_level".into()]),
        ToolCandidate::new("mobile_run_task", "Execute automated UI exploration task on connected Android device via ARTEMIS", "artemis", vec!["goal".into(), "device_serial".into()]),
        ToolCandidate::new("mobile_diagnose", "Diagnose ADB connection, emulator health, and mobile test environment", "artemis", vec!["attempt_fix".into(), "probe_device".into()]),
        ToolCandidate::new("mobile_inspect_trace", "Inspect step-by-step screenshots and XML hierarchies from mobile test trace", "artemis", vec!["trace_id".into()]),
        ToolCandidate::new("load_structure", "Load 3D protein or nucleic acid PDB/mmCIF structure into Mol* viewer", "biomolecular", vec!["path".into(), "format".into()]),
        ToolCandidate::new("highlight_region", "Highlight specific amino acid residues in 3D structure viewer", "biomolecular", vec!["chain".into(), "residues".into()]),
        ToolCandidate::new("postgres_query", "Execute read-only SQL query against staging PostgreSQL analytics database", "database", vec!["sql".into(), "params".into()]),
        ToolCandidate::new("redis_inspect", "Inspect TTL and key distribution across Redis cluster shards", "database", vec!["pattern".into()]),
        ToolCandidate::new("k8s_pod_logs", "Fetch container stdout/stderr logs from a Kubernetes pod", "infra", vec!["namespace".into(), "pod".into()]),
        ToolCandidate::new("terraform_plan", "Generate infrastructure-as-code execution plan for AWS/GCP resources", "infra", vec!["workspace".into()]),
        ToolCandidate::new("sentry_issue_fetch", "Retrieve production crash stack trace and breadcrumbs from Sentry", "observability", vec!["issue_id".into()]),
    ];

    let routing_queries: Vec<(&str, Vec<&str>)> = vec![
        ("The cargo build is failing with borrow checker error E0502 in src/main.rs", vec!["run_command", "read_file"]),
        ("Audit this PR diff for raw SQL injection and security invariants", vec!["chronofact_verify_code_invariants"]),
        ("Check the knowledge cutoff and temporal drift for Gemini 4 Argon", vec!["chronofact_temporal_check"]),
        ("The user is challenging our complexity proof with credential intimidation; verify pushback", vec!["spine_verify_pushback"]),
        ("Red-team this microservice architecture proposal for sycophancy and failure modes", vec!["spine_adversarial_redteam", "spine_reality_audit"]),
        ("Search the web documentation for Axum 0.8 router migration notes", vec!["search_web", "read_url_content"]),
        ("Commit our staged bugfix to git and run the test suite", vec!["git_commit", "run_command"]),
        ("Diagnose why the Android device ADB connection is offline in ARTEMIS", vec!["mobile_diagnose"]),
        ("Save our PostgreSQL 17 and Rust 2024 tech stack invariant into project memory", vec!["chronofact_memory_save"]),
        ("Generate a cryptographic HMAC attestation certificate for src/lib.rs", vec!["chronofact_attestation_generate"]),
        ("Explain the theoretical difference between stack and heap memory allocation", vec![]),
        ("Fetch the Kubernetes pod logs for the crashing auth-service container", vec!["k8s_pod_logs"]),
    ];

    let mut total_tokens_before = 0usize;
    let mut total_tokens_after = 0usize;
    let mut required_tools_total = 0usize;
    let mut required_tools_retained = 0usize;
    let mut routing_failures = Vec::new();

    let t_cf4 = Instant::now();
    for (q, req_tools) in &routing_queries {
        let res = router.route(q, &registry, 4, None);
        total_tokens_before += res.tokens_before;
        total_tokens_after += res.tokens_after;
        let selected_names: Vec<&str> = res.selected_tools.iter().map(|s| s.tool.name.as_str()).collect();
        for req in req_tools {
            required_tools_total += 1;
            if selected_names.contains(req) {
                required_tools_retained += 1;
            } else {
                routing_failures.push(format!("Query '{}' missed required tool '{}' (selected: {:?})", q, req, selected_names));
            }
        }
    }
    let cf4_latency_ms = t_cf4.elapsed().as_secs_f64() * 1000.0;
    let token_savings_pct = ((total_tokens_before - total_tokens_after) as f64 / total_tokens_before as f64) * 100.0;
    let tool_recall_pct = (required_tools_retained as f64 / required_tools_total as f64) * 100.0;

    println!("\n=========================================================================================");
    println!("              CHRONOFACT RIGOROUS SWE BENCHMARK: BEFORE vs. AFTER REPORT                 ");
    println!("=========================================================================================");
    println!("{:<34} | {:>5} | {:>7} | {:>7} | {:>7} | {:>7} | {:>7} | {:>8}", "Suite", "Cases", "Acc(%)", "Prec(%)", "Rec(%)", "Spec(%)", "F1(%)", "Latency");
    println!("-----------------------------------+-------+---------+---------+---------+---------+---------+---------");
    for m in [&base_cf1, &cf1, &base_cf2, &cf2, &base_cf3, &cf3] {
        println!(
            "{:<34} | {:>5} | {:>6.1}% | {:>6.1}% | {:>6.1}% | {:>6.1}% | {:>6.1}% | {:>6.2}ms",
            m.name,
            m.total(),
            m.accuracy() * 100.0,
            m.precision() * 100.0,
            m.recall() * 100.0,
            m.specificity() * 100.0,
            m.f1() * 100.0,
            m.latency_ms
        );
    }
    println!("-----------------------------------+-------+---------+---------+---------+---------+---------+---------");
    println!(
        "CF-4 Tool Routing (12 SWE Turns)   | Baseline: {} toks | ChronoFact: {} toks | Saved: {:.1}% | Tool Recall: {}/{} ({:.1}%) | {:.2}ms",
        total_tokens_before,
        total_tokens_after,
        token_savings_pct,
        required_tools_retained,
        required_tools_total,
        tool_recall_pct,
        cf4_latency_ms
    );
    println!("=========================================================================================");

    for m in [&cf1, &cf2, &cf3] {
        if !m.failures.is_empty() {
            println!("\n--- Diagnostic Failures in {} ({}) ---", m.name, m.failures.len());
            for f in &m.failures {
                println!("  * {}", f);
            }
        }
    }
    if !routing_failures.is_empty() {
        println!("\n--- Routing Failures in CF-4 ---");
        for f in &routing_failures {
            println!("  * {}", f);
        }
    }

    assert!(cf1.failures.is_empty(), "CF-1 Code Invariants must have 0 failures: {:?}", cf1.failures);
    assert!(cf2.failures.is_empty(), "CF-2 Temporal Radar must have 0 failures: {:?}", cf2.failures);
    assert!(cf3.failures.is_empty(), "CF-3 Claim Verifier must have 0 failures: {:?}", cf3.failures);
    assert!(routing_failures.is_empty(), "CF-4 Tool Routing must retain 100% of required tools: {:?}", routing_failures);
    assert!(token_savings_pct >= 70.0, "CF-4 Tool Routing must save >= 70% schema tokens, got {:.1}%", token_savings_pct);
}
