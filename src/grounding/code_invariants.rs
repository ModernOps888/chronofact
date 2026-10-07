//! Code & Architectural Invariant Engine
//!
//! Evaluates source code, configuration manifests, and Git diffs against
//! enterprise architectural invariants, security boundaries, and temporal deprecation
//! policies without requiring heavy external linters.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvariantSeverity {
    Critical, // Blocks CI/CD deployment immediately
    High,     // Blocks deployment unless explicit override granted
    Medium,   // Warning requiring architectural review note
    Low,      // Advisory notice
}

impl InvariantSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "CRITICAL",
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantRule {
    pub id: String,
    pub name: String,
    pub category: String, // "Security", "Architecture", "TemporalDeprecation", "Reliability"
    pub severity: InvariantSeverity,
    pub description: String,
    pub pattern: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantViolation {
    pub rule_id: String,
    pub rule_name: String,
    pub category: String,
    pub severity: InvariantSeverity,
    pub file_path: String,
    pub line_number: usize,
    pub line_snippet: String,
    pub message: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditVerdict {
    Pass,
    Advisory,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAuditReport {
    pub target: String,
    pub total_lines_audited: usize,
    pub total_rules_evaluated: usize,
    pub violations_count: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub verdict: AuditVerdict,
    pub violations: Vec<InvariantViolation>,
    pub execution_duration_ms: f64,
}

pub struct CodeInvariantChecker {
    built_in_rules: Vec<InvariantRule>,
}

impl Default for CodeInvariantChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeInvariantChecker {
    pub fn new() -> Self {
        let rules = vec![
            InvariantRule {
                id: "INV-SEC-RAW-SQL".to_string(),
                name: "Unparameterized Raw SQL Interpolation".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::Critical,
                description: "Detects direct string concatenation or formatting inside SQL queries, risking SQL injection.".to_string(),
                pattern: r#"(?i)(?:format!\s*\(\s*["'].*(?:SELECT|INSERT|UPDATE|DELETE|DROP|ALTER).*\{|\b(?:SELECT|INSERT|UPDATE|DELETE)\b.*(?:\$\{.*\}|\+\s*[a-zA-Z_]))"#.to_string(),
                remediation: "Use parameterized queries ($1, ?), prepared statements, or an approved query builder/ORM.".to_string(),
            },
            InvariantRule {
                id: "INV-SEC-EVAL-EXEC".to_string(),
                name: "Dynamic Code Execution / Dangerous HTML".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::Critical,
                description: "Detects arbitrary code execution primitives (eval, exec) or unsanitized DOM rendering.".to_string(),
                pattern: r#"\b(?:eval\s*\(|exec\s*\(|dangerouslySetInnerHTML|child_process\.exec\s*\()"#.to_string(),
                remediation: "Avoid runtime string code evaluation; parse structured payloads explicitly and sanitize DOM nodes.".to_string(),
            },
            InvariantRule {
                id: "INV-SEC-HARDCODED-SECRET".to_string(),
                name: "Hardcoded API Key / Secret Token".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::Critical,
                description: "Detects hardcoded high-entropy tokens, AWS keys, or API credentials.".to_string(),
                pattern: r#"(?:AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9_]{36}|sk-[a-zA-Z0-9]{32,}|(?i)(?:api_key|secret_key|private_key|auth_token)\s*[:=]\s*["'][A-Za-z0-9_\-\.]{16,}["'])"#.to_string(),
                remediation: "Extract secrets into secure environment variables, cloud secrets vaults, or KMS.".to_string(),
            },
            InvariantRule {
                id: "INV-ARCH-DAL-LEAK".to_string(),
                name: "Presentation Layer Direct Database Access".to_string(),
                category: "Architecture".to_string(),
                severity: InvariantSeverity::High,
                description: "Detects direct database client imports within client/presentation layer files.".to_string(),
                pattern: r#"(?i)(?:import.*from\s*["'](?:rusqlite|@prisma/client|mysql2|pg|typeorm|mongoose)["']|use\s+(?:rusqlite|sqlx|diesel)::)"#.to_string(),
                remediation: "Route database calls through domain services, repository interfaces, or API gateway endpoints.".to_string(),
            },
            InvariantRule {
                id: "INV-TEMP-DEPRECATED-LIB".to_string(),
                name: "Deprecated / EOL Dependency Import".to_string(),
                category: "TemporalDeprecation".to_string(),
                severity: InvariantSeverity::High,
                description: "Detects imports of libraries sunsetted or replaced by modern 2026 enterprise standards.".to_string(),
                pattern: r#"(?:from\s*["'](?:moment|request|urllib3)["']|require\(["'](?:moment|request)["']\))"#.to_string(),
                remediation: "Replace with current standards: moment -> date-fns/Luxon; request -> fetch/axios/reqwest.".to_string(),
            },
            InvariantRule {
                id: "INV-SEC-SSRF-UNCHECKED".to_string(),
                name: "Unvalidated Outbound HTTP Call (SSRF Risk)".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::Medium,
                description: "Detects direct outbound fetch/request using raw request parameters without IP/URL validation.".to_string(),
                pattern: r#"(?:fetch\s*\(\s*(?:req\.params|req\.query|req\.body|params\.)|reqwest::get\s*\(\s*&?url\b)"#.to_string(),
                remediation: "Validate URLs against private IP ranges (RFC 1918, link-local) before dispatching outbound requests.".to_string(),
            },
            InvariantRule {
                id: "INV-ERR-SILENT-FAIL".to_string(),
                name: "Silent Exception Swallowing".to_string(),
                category: "Reliability".to_string(),
                severity: InvariantSeverity::Medium,
                description: "Detects empty catch blocks or discarded Results that hide runtime crashes.".to_string(),
                pattern: r#"catch\s*\([a-zA-Z0-9_]*\)\s*\{\s*\}|let\s+_\s*=\s*(?:std::fs|tokio::fs|rusqlite)"#.to_string(),
                remediation: "Log errors with structured context or propagate with explicit error types.".to_string(),
            },
        ];

        Self {
            built_in_rules: rules,
        }
    }

    /// Audits code content and returns a structured CodeAuditReport.
    pub fn audit_code(
        &self,
        file_path: &str,
        content: &str,
        custom_rules: Option<&[InvariantRule]>,
    ) -> CodeAuditReport {
        let start = Instant::now();
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let mut all_rules = self.built_in_rules.clone();
        if let Some(custom) = custom_rules {
            all_rules.extend_from_slice(custom);
        }

        let mut compiled_rules = Vec::new();
        for r in &all_rules {
            if let Ok(re) = Regex::new(&r.pattern) {
                compiled_rules.push((r, re));
            }
        }

        let total_rules = compiled_rules.len();
        let mut violations = Vec::new();

        // Special check: Architecture DAL leak only triggers on client/frontend/ui paths
        let is_frontend_path = file_path.contains("frontend")
            || file_path.contains("client")
            || file_path.contains("ui")
            || file_path.contains("components")
            || file_path.contains("pages");

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") {
                continue;
            }

            for (rule, regex) in &compiled_rules {
                if rule.id == "INV-ARCH-DAL-LEAK" && !is_frontend_path {
                    continue; // Skip DAL leak check on backend files
                }

                if regex.is_match(line) {
                    violations.push(InvariantViolation {
                        rule_id: rule.id.clone(),
                        rule_name: rule.name.clone(),
                        category: rule.category.clone(),
                        severity: rule.severity,
                        file_path: file_path.to_string(),
                        line_number: line_num,
                        line_snippet: trimmed.to_string(),
                        message: format!("Violated {}: {}", rule.name, rule.description),
                        remediation: rule.remediation.clone(),
                    });
                }
            }
        }

        let mut critical_count = 0;
        let mut high_count = 0;
        let mut medium_count = 0;
        let mut low_count = 0;

        for v in &violations {
            match v.severity {
                InvariantSeverity::Critical => critical_count += 1,
                InvariantSeverity::High => high_count += 1,
                InvariantSeverity::Medium => medium_count += 1,
                InvariantSeverity::Low => low_count += 1,
            }
        }

        let verdict = if critical_count > 0 || high_count > 0 {
            AuditVerdict::Block
        } else if medium_count > 0 || low_count > 0 {
            AuditVerdict::Advisory
        } else {
            AuditVerdict::Pass
        };

        CodeAuditReport {
            target: file_path.to_string(),
            total_lines_audited: total_lines,
            total_rules_evaluated: total_rules,
            violations_count: violations.len(),
            critical_count,
            high_count,
            medium_count,
            low_count,
            verdict,
            violations,
            execution_duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        }
    }
}
