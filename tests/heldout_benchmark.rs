//! Blind Out-of-Sample Held-Out SWE Benchmark for Project ChronoFact (140 Cases)
//!
//! Evaluates the frozen blind held-out JSON datasets authored without reading
//! ChronoFact source code or existing test suites:
//! - `tests/heldout/cf1_code_invariants.json` (N = 60: 30 flag / 30 clean)
//! - `tests/heldout/cf2_temporal_radar.json`  (N = 40: 20 grounding / 20 timeless)
//! - `tests/heldout/cf3_claim_verifier.json`  (N = 40 responses / 59 claims)

use chronofact::{
    AuditVerdict, ClaimExtractor, CodeInvariantChecker, FactVerifier, TemporalScanner,
    VerificationStatus,
};
use serde::Deserialize;
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct Cf1Case {
    id: String,
    language: String,
    category: String,
    expected: String,
    code: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct Cf2Case {
    id: String,
    prompt: String,
    expected_requires_grounding: bool,
    #[allow(dead_code)]
    expected_outdated_entities: Vec<String>,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct Cf3ExpectedClaim {
    #[allow(dead_code)]
    text: String,
    expected: String,
}

#[derive(Debug, Deserialize)]
struct Cf3Case {
    id: String,
    response_text: String,
    claims: Vec<Cf3ExpectedClaim>,
    rationale: String,
}

#[derive(Debug, Clone)]
struct Metrics {
    name: &'static str,
    tp: usize,
    tn: usize,
    fp: usize,
    fn_count: usize,
    latency_ms: f64,
    failures: Vec<String>,
}

impl Metrics {
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
    fn acc(&self) -> f64 {
        (self.tp + self.tn) as f64 / self.total().max(1) as f64 * 100.0
    }
    fn prec(&self) -> f64 {
        if self.tp + self.fp == 0 {
            0.0
        } else {
            self.tp as f64 / (self.tp + self.fp) as f64 * 100.0
        }
    }
    fn rec(&self) -> f64 {
        if self.tp + self.fn_count == 0 {
            0.0
        } else {
            self.tp as f64 / (self.tp + self.fn_count) as f64 * 100.0
        }
    }
    fn spec(&self) -> f64 {
        if self.tn + self.fp == 0 {
            0.0
        } else {
            self.tn as f64 / (self.tn + self.fp) as f64 * 100.0
        }
    }
    fn f1(&self) -> f64 {
        let p = self.prec();
        let r = self.rec();
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
}

fn synthetic_file_path(lang: &str, category: &str) -> &'static str {
    if category.contains("dal") || category.contains("layer") {
        match lang {
            "python" => "src/views/user_controller.py",
            "javascript" | "typescript" | "tsx" | "jsx" => "src/components/UserView.tsx",
            "go" => "src/handlers/user_controller.go",
            "rust" => "src/ui/user_view.rs",
            _ => "src/ui/controller.ts",
        }
    } else {
        match lang {
            "rust" => "src/lib.rs",
            "python" => "src/app.py",
            "javascript" => "src/index.js",
            "typescript" => "src/index.ts",
            "tsx" | "jsx" | "react" => "src/Component.tsx",
            "go" => "src/main.go",
            "java" => "src/Main.java",
            "c" | "cpp" | "cuda" => "src/kernel.cu",
            "sql" => "migrations/001.sql",
            "shell" | "bash" | "sh" | "powershell" => "scripts/deploy.sh",
            _ => "src/main.txt",
        }
    }
}

#[test]
fn test_chronofact_blind_heldout_benchmark() {
    // 1. CF-1 Held-Out (60 cases)
    let cf1_raw = include_str!("heldout/cf1_code_invariants.json")
        .replace("{{STRIPE_LIVE_PREFIX}}", &format!("{}_{}_", "sk", "live"));
    let cf1_cases: Vec<Cf1Case> = serde_json::from_str(&cf1_raw).expect("parse cf1 json");
    let checker = CodeInvariantChecker::new();
    let mut m_cf1 = Metrics::new("CF-1 Held-Out Code Invariants");
    let t1 = Instant::now();
    for c in &cf1_cases {
        let expect_flag = c.expected == "flag";
        let path = synthetic_file_path(&c.language, &c.category);
        let res = checker.audit_code(path, &c.code, None);
        let actual_flag = res.verdict != AuditVerdict::Pass || !res.violations.is_empty();
        if expect_flag && actual_flag {
            m_cf1.tp += 1;
        } else if !expect_flag && !actual_flag {
            m_cf1.tn += 1;
        } else if !expect_flag && actual_flag {
            m_cf1.fp += 1;
            let rules: Vec<_> = res.violations.iter().map(|v| v.rule_id.as_str()).collect();
            m_cf1.failures.push(format!(
                "[FP] {} ({}/{}) rules={:?} | {}",
                c.id, c.language, c.category, rules, c.rationale
            ));
        } else {
            m_cf1.fn_count += 1;
            m_cf1.failures.push(format!(
                "[FN] {} ({}/{}) | {} | code={:?}",
                c.id, c.language, c.category, c.rationale, c.code
            ));
        }
    }
    m_cf1.latency_ms = t1.elapsed().as_secs_f64() * 1000.0;

    // 2. CF-2 Held-Out (40 cases)
    let cf2_raw = include_str!("heldout/cf2_temporal_radar.json");
    let cf2_cases: Vec<Cf2Case> = serde_json::from_str(cf2_raw).expect("parse cf2 json");
    let scanner = TemporalScanner::new();
    let mut m_cf2 = Metrics::new("CF-2 Held-Out Temporal Radar");
    let t2 = Instant::now();
    for c in &cf2_cases {
        let scan = scanner.scan(&c.prompt);
        let actual_grounding = scan.is_temporally_sensitive || !scan.outdated_models_flagged.is_empty() || scan.temporal_risk_score >= 0.35;
        if c.expected_requires_grounding && actual_grounding {
            m_cf2.tp += 1;
        } else if !c.expected_requires_grounding && !actual_grounding {
            m_cf2.tn += 1;
        } else if !c.expected_requires_grounding && actual_grounding {
            m_cf2.fp += 1;
            m_cf2.failures.push(format!(
                "[FP] {} score={:.2} entities={:?} kw={:?} | {} | prompt={:?}",
                c.id, scan.temporal_risk_score, scan.detected_entities, scan.temporal_keywords, c.rationale, c.prompt
            ));
        } else {
            m_cf2.fn_count += 1;
            m_cf2.failures.push(format!(
                "[FN] {} score={:.2} entities={:?} kw={:?} | {} | prompt={:?}",
                c.id, scan.temporal_risk_score, scan.detected_entities, scan.temporal_keywords, c.rationale, c.prompt
            ));
        }
    }
    m_cf2.latency_ms = t2.elapsed().as_secs_f64() * 1000.0;

    // 3. CF-3 Held-Out (40 responses)
    let cf3_raw = include_str!("heldout/cf3_claim_verifier.json");
    let cf3_cases: Vec<Cf3Case> = serde_json::from_str(cf3_raw).expect("parse cf3 json");
    let extractor = ClaimExtractor::new();
    let verifier = FactVerifier::new();
    let mut m_cf3 = Metrics::new("CF-3 Held-Out Claim Verifier");
    let t3 = Instant::now();
    for c in &cf3_cases {
        let expect_contradicted = c.claims.iter().any(|cl| cl.expected == "contradicted");
        let extracted = extractor.extract_claims(&c.response_text);
        let report = verifier.verify_claims(&extracted, &[]);
        let actual_contradicted = report.contradicted_count > 0
            || report
                .claims
                .iter()
                .any(|vc| vc.status == VerificationStatus::Contradicted);
        if expect_contradicted && actual_contradicted {
            m_cf3.tp += 1;
        } else if !expect_contradicted && !actual_contradicted {
            m_cf3.tn += 1;
        } else if !expect_contradicted && actual_contradicted {
            m_cf3.fp += 1;
            m_cf3.failures.push(format!(
                "[FP] {} | {} | resp={:?}",
                c.id, c.rationale, c.response_text
            ));
        } else {
            m_cf3.fn_count += 1;
            m_cf3.failures.push(format!(
                "[FN] {} | {} | resp={:?}",
                c.id, c.rationale, c.response_text
            ));
        }
    }
    m_cf3.latency_ms = t3.elapsed().as_secs_f64() * 1000.0;

    println!("\n=========================================================================================");
    println!("              CHRONOFACT BLIND HELD-OUT BENCHMARK REPORT (140 CASES)                     ");
    println!("=========================================================================================");
    println!(
        "{:<34} | {:>5} | {:>7} | {:>7} | {:>7} | {:>7} | {:>7} | {:>8}",
        "Pillar", "Cases", "Acc(%)", "Prec(%)", "Rec(%)", "Spec(%)", "F1(%)", "Latency"
    );
    println!("-----------------------------------+-------+---------+---------+---------+---------+---------+---------");
    for m in [&m_cf1, &m_cf2, &m_cf3] {
        println!(
            "{:<34} | {:>5} | {:>6.1}% | {:>6.1}% | {:>6.1}% | {:>6.1}% | {:>6.1}% | {:>6.2}ms",
            m.name,
            m.total(),
            m.acc(),
            m.prec(),
            m.rec(),
            m.spec(),
            m.f1(),
            m.latency_ms
        );
    }
    println!("=========================================================================================");

    for m in [&m_cf1, &m_cf2, &m_cf3] {
        if !m.failures.is_empty() {
            println!("\n--- Diagnostic Failures in {} ({}) ---", m.name, m.failures.len());
            for f in &m.failures {
                println!("  * {}", f);
            }
        }
    }

    assert!(m_cf1.failures.is_empty(), "CF-1 Held-Out failures: {:?}", m_cf1.failures);
    assert!(m_cf2.failures.is_empty(), "CF-2 Held-Out failures: {:?}", m_cf2.failures);
    assert!(m_cf3.failures.is_empty(), "CF-3 Held-Out failures: {:?}", m_cf3.failures);
}
