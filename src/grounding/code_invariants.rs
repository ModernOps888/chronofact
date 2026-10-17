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
    built_in_rules: Vec<(InvariantRule, Regex)>,
}

impl Default for CodeInvariantChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeInvariantChecker {
    pub fn new() -> Self {
        static BUILT_IN_RULES: std::sync::OnceLock<Vec<(InvariantRule, Regex)>> =
            std::sync::OnceLock::new();
        let built_in_rules = BUILT_IN_RULES
            .get_or_init(|| {
                let rules = vec![
            InvariantRule {
                id: "INV-SEC-RAW-SQL".to_string(),
                name: "Unparameterized Raw SQL Interpolation".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::Critical,
                description: "Detects direct string concatenation or formatting inside SQL queries across Rust, TypeScript/JS, Python, and Go, risking SQL injection.".to_string(),
                pattern: r#"(?i)(?:format!\s*\(\s*"[^"]*\b(?:SELECT\s+[^"]*\bFROM|INSERT\s+INTO|UPDATE\s+\w+\s+SET|DELETE\s+FROM|DROP\s+TABLE|ALTER\s+TABLE)\b[^"]*\{|fmt\.Sprintf\s*\(\s*"[^"]*\b(?:SELECT\s+[^"]*\bFROM|INSERT\s+INTO|UPDATE\s+\w+\s+SET|DELETE\s+FROM)\b[^"]*%[svqd]|\b(?:SELECT\s+.*\bFROM|INSERT\s+INTO|UPDATE\s+\w+\s+SET|DELETE\s+FROM)\b.*(?:\$\{.*\}|\+\s*[a-zA-Z_])|[rb]?f(?:"[^"]*\b(?:select\s+.*\bfrom|insert\s+into|update\s+\w+\s+set|delete\s+from|drop\s+table|alter\s+table)\b[^"]*\{|'[^']*\b(?:select\s+.*\bfrom|insert\s+into|update\s+\w+\s+set|delete\s+from|drop\s+table|alter\s+table)\b[^']*\{)|\b(?:execute|executemany|executescript|query|raw|read_sql)\s*\(\s*[rb]?f(?:"[^"]*\b(?:select|insert|update|delete|drop|alter)\b[^"]*\{|'[^']*\b(?:select|insert|update|delete|drop|alter)\b[^']*\{)|\b(?:execute|executemany|executescript|query|raw|read_sql)\s*\(\s*(?:"[^"]*\b(?:select|insert|update|delete|drop|alter)\b[^"]*"|'[^']*\b(?:select|insert|update|delete|drop|alter)\b[^']*')\s*(?:%\s*[\w(\[]|\.format\s*\())"#.to_string(),
                remediation: "Use parameterized queries ($1, ?), prepared statements, or an approved query builder/ORM.".to_string(),
            },
            InvariantRule {
                id: "INV-SEC-EVAL-EXEC".to_string(),
                name: "Dynamic Code Execution / Command Injection / Dangerous HTML".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::Critical,
                description: "Detects arbitrary code execution primitives (eval, exec, Invoke-Expression), shell injection, or unsanitized DOM rendering.".to_string(),
                pattern: r#"(?:\beval\s*\(|(?:^|[^.\w])exec\s*\(|\bnew\s+Function\s*\(|(?:^|[^.\w])Function\s*\(|\bos\.system\s*\(|dangerouslySetInnerHTML|child_process\.exec\s*\(|\bsubprocess\.\w+\s*\([^)]*\bshell\s*=\s*True|\bInvoke-Expression\b|\biex\s+\$|\bset(?:Timeout|Interval)\s*\(\s*(?:"[^"]*"|'[^']*'|`[^`]*`)\s*[,)])"#.to_string(),
                remediation: "Avoid runtime string code evaluation; use parameterized argument arrays without shell=True and sanitize DOM nodes.".to_string(),
            },
            InvariantRule {
                id: "INV-SEC-HARDCODED-SECRET".to_string(),
                name: "Hardcoded API Key / Secret Token / Private Key".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::Critical,
                description: "Detects hardcoded high-entropy tokens, AWS keys, GitHub PATs, OpenRouter/OpenAI keys, JWTs, or PEM private keys.".to_string(),
                pattern: r#"(?:AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9_]{36}|sk-(?:or-v1-)?[a-zA-Z0-9]{32,}|\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b|-----BEGIN\s+(?:RSA\s+|EC\s+|OPENSSH\s+)?PRIVATE\s+KEY-----|(?i)(?:api_key|secret_key|private_key|auth_token)\s*[:=]\s*["'][A-Za-z0-9_\-\.]{16,}["'])"#.to_string(),
                remediation: "Extract secrets into secure environment variables, cloud secrets vaults, or KMS.".to_string(),
            },
            InvariantRule {
                id: "INV-SEC-MEMORY-UNSAFE".to_string(),
                name: "Memory-Unsafe C/C++/CUDA Buffer Call or Unguarded Rust Transmute".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::High,
                description: "Detects banned unbounded C/C++/CUDA buffer functions (strcpy, strcat, sprintf, gets) or unguarded Rust std::mem::transmute / from_utf8_unchecked.".to_string(),
                pattern: r#"(?:\b(?:strcpy|strcat|sprintf|vsprintf|gets)\s*\(|\bstd::mem::transmute\b|\bfrom_utf8_unchecked\s*\()"#.to_string(),
                remediation: "In C/C++/CUDA use bounds-checked snprintf/strncpy; in Rust use safe conversions (TryFrom, bytemuck, std::str::from_utf8).".to_string(),
            },
            InvariantRule {
                id: "INV-SEC-PATH-TRAVERSAL".to_string(),
                name: "Unvalidated Path Traversal or Insecure Deserialization".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::High,
                description: "Detects direct filesystem reads from untrusted request parameters or arbitrary object deserialization (pickle.loads, yaml.load without SafeLoader, marshal.loads).".to_string(),
                pattern: r#"(?:\b(?:pickle|marshal|shelve)\.loads?\s*\(|\byaml\.load\s*\([^,)]+\)|(?:fs::read|fs\.readFile|fs\.readFileSync|http\.ServeFile)\s*\([^)]*(?:req\.|request\.|params\.|query\.))"#.to_string(),
                remediation: "Canonicalize and restrict file paths within a base directory; use yaml.safe_load or structured JSON schemas instead of pickle.".to_string(),
            },
            InvariantRule {
                id: "INV-SEC-WEAK-CRYPTO-TLS".to_string(),
                name: "Disabled TLS Certificate Validation or Broken Cryptographic Hash".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::High,
                description: "Detects disabled TLS verification across Rust, TypeScript/Node, Python, and Go, or MD5/SHA1 password hashing.".to_string(),
                pattern: r#"(?:danger_accept_invalid_certs\s*\(\s*true\s*\)|rejectUnauthorized\s*:\s*false|NODE_TLS_REJECT_UNAUTHORIZED\s*=\s*["']0["']|InsecureSkipVerify\s*:\s*true|\bverify\s*=\s*False\b|(?i)(?:md5|sha1)\s*\([^)]*password)"#.to_string(),
                remediation: "Enforce strict TLS certificate verification and use Argon2id, bcrypt, or SHA-256/HMAC-SHA256.".to_string(),
            },
            InvariantRule {
                id: "INV-ARCH-DAL-LEAK".to_string(),
                name: "Presentation Layer Direct Database Access".to_string(),
                category: "Architecture".to_string(),
                severity: InvariantSeverity::High,
                description: "Detects direct database client imports within client/presentation layer files.".to_string(),
                pattern: r#"(?i)(?:import.*from\s*["'](?:rusqlite|@prisma/client|mysql2|pg|typeorm|mongoose|better-sqlite3)["']|use\s+(?:rusqlite|sqlx|diesel)::)"#.to_string(),
                remediation: "Route database calls through domain services, repository interfaces, or API gateway endpoints.".to_string(),
            },
            InvariantRule {
                id: "INV-TEMP-DEPRECATED-LIB".to_string(),
                name: "Deprecated / EOL Dependency, Retired AI Model, or Vendor Misattribution".to_string(),
                category: "TemporalDeprecation".to_string(),
                severity: InvariantSeverity::High,
                description: "Detects sunsetted libraries (moment, request, ioutil.ReadAll), retired pre-2026 AI model IDs, or vendor misattributions.".to_string(),
                pattern: r#"(?:from\s*["'](?:moment|request|urllib3)["']|require\(["'](?:moment|request)["']\)|\bioutil\.(?:ReadAll|ReadFile|WriteFile|TempDir|TempFile)\b|(?i)["'](?:claude-3-5-(?:sonnet|haiku)(?:-\d{8}|-latest)?|claude-3-(?:opus|sonnet|haiku)(?:-\d{8}|-latest)?|claude-3-7-sonnet(?:-\d{8}|-latest)?|gpt-4-(?:0613|0314|turbo-preview|1106-preview)|text-davinci-00[23]|grok-[23](?:-mini|-latest)?|(?:google|anthropic)/(?:astra-6|sol-6\.1))["'])"#.to_string(),
                remediation: "Replace with active 2026 standards: moment -> date-fns/Luxon; request -> fetch/reqwest; ioutil -> io/os; retired models -> claude-sonnet-5-5 / claude-opus-5-5 / openai/gpt-6-astra / google/gemini-4-argon / x-ai/grok-4.7.".to_string(),
            },
            InvariantRule {
                id: "INV-TEMP-HALLUCINATED-API".to_string(),
                name: "Hallucinated Standard Library or Framework Method".to_string(),
                category: "TemporalDeprecation".to_string(),
                severity: InvariantSeverity::High,
                description: "Detects non-existent methods commonly hallucinated by LLMs across Rust, TypeScript/JS, Python, and Go.".to_string(),
                pattern: r#"(?:\.(?:is_none_or_empty|to_str_lossy|flatMapAsync)\s*\(|\btokio::time::delay_for\s*\(|\bJSON\.tryParse\s*\(|\bPromise\.waitAll\s*\(|\basyncio\.sleep_ms\s*\(|\bstrings\.ContainsAnyWord\s*\()"#.to_string(),
                remediation: "Use verified standard library APIs: Rust Option::is_none / String::from_utf8_lossy / tokio::time::sleep; JS JSON.parse / Promise.all / Promise.allSettled; Python asyncio.sleep; Go strings.ContainsAny.".to_string(),
            },
            InvariantRule {
                id: "INV-SEC-SSRF-UNCHECKED".to_string(),
                name: "Unvalidated Outbound HTTP Call (SSRF Risk)".to_string(),
                category: "Security".to_string(),
                severity: InvariantSeverity::Medium,
                description: "Detects direct outbound fetch/request using raw request parameters without IP/URL validation.".to_string(),
                pattern: r#"(?:fetch\s*\(\s*(?:req\.params|req\.query|req\.body|params\.)|reqwest::get\s*\(\s*&?url\b|requests\.get\s*\(\s*request\.(?:args|form|json)|http\.Get\s*\(\s*r\.URL\.Query)"#.to_string(),
                remediation: "Validate URLs against private IP ranges (RFC 1918, link-local) before dispatching outbound requests.".to_string(),
            },
            InvariantRule {
                id: "INV-ERR-SILENT-FAIL".to_string(),
                name: "Silent Exception Swallowing".to_string(),
                category: "Reliability".to_string(),
                severity: InvariantSeverity::Medium,
                description: "Detects empty catch/except blocks or discarded Results that hide runtime crashes.".to_string(),
                pattern: r#"(?:catch\s*\([a-zA-Z0-9_]*\)\s*\{\s*\}|except(?:\s+Exception)?\s*:\s*pass\b|let\s+_\s*=\s*(?:std::fs|tokio::fs|rusqlite))"# .to_string(),
                remediation: "Log errors with structured context or propagate with explicit error types.".to_string(),
            },
            InvariantRule {
                id: "INV-REL-ASYNC-BLOCKING".to_string(),
                name: "Blocking Thread Sleep / Sync I/O Inside Async Runtime".to_string(),
                category: "Reliability".to_string(),
                severity: InvariantSeverity::Medium,
                description: "Detects std::thread::sleep or synchronous child_process.execSync calls that starve async Tokio/Node worker threads.".to_string(),
                pattern: r#"(?:\bstd::thread::sleep\s*\(|\bchild_process\.execSync\s*\()"#.to_string(),
                remediation: "Use tokio::time::sleep in async Rust or async child_process.execFile / spawn in Node.js.".to_string(),
            },
        ];

                rules
                    .into_iter()
                    .filter_map(|r| Regex::new(&r.pattern).ok().map(|re| (r, re)))
                    .collect()
            })
            .clone();

        Self { built_in_rules }
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

        let mut compiled_custom: Vec<(InvariantRule, Regex)> = Vec::new();
        if let Some(custom) = custom_rules {
            for r in custom {
                if let Ok(re) = Regex::new(&r.pattern) {
                    compiled_custom.push((r.clone(), re));
                }
            }
        }

        let total_rules = self.built_in_rules.len() + compiled_custom.len();
        let mut violations = Vec::new();

        // Special check: Architecture DAL leak only triggers on client/frontend/ui paths
        // Path-segment match (not substring): "build/guide.rs" or "quiz.rs" must not count as UI code.
        let is_frontend_path = file_path
            .to_lowercase()
            .split(|c| c == '/' || c == '\\')
            .any(|seg| matches!(seg, "frontend" | "client" | "ui" | "components" | "pages" | "app" | "views" | "src-ui"));

        let has_any_fn_decl = content.contains("fn ") || content.contains("function ") || content.contains("def ");
        let mut in_block_comment = false;
        let mut in_py_docstring = false;
        let mut in_async_scope = !has_any_fn_decl;
        let mut sql_window_skip_until: usize = 0;

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            // Track multi-line /* ... */ block comments statefully
            if in_block_comment {
                if let Some(end_pos) = trimmed.find("*/") {
                    in_block_comment = false;
                    let after = trimmed[end_pos + 2..].trim();
                    if after.is_empty() {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            if trimmed.starts_with("/*") {
                if !trimmed[2..].contains("*/") {
                    in_block_comment = true;
                }
                continue;
            }

            // Track multi-line Python """ ... """ or ''' ... ''' docstrings statefully
            let triple_delim = if trimmed.contains("\"\"\"") {
                Some("\"\"\"")
            } else if trimmed.contains("'''") {
                Some("'''")
            } else {
                None
            };
            if in_py_docstring {
                if triple_delim.is_some() {
                    in_py_docstring = false;
                }
                continue;
            }
            if let Some(delim) = triple_delim {
                if trimmed.starts_with(delim) {
                    let rest = &trimmed[delim.len()..];
                    if !rest.contains(delim) {
                        in_py_docstring = true;
                    }
                    continue;
                }
            }

            if trimmed.is_empty()
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with("--")
                || trimmed == "*"
                || trimmed.starts_with("* ")
                || trimmed.starts_with("*/")
            {
                continue;
            }

            // Track async vs synchronous function scope for INV-REL-ASYNC-BLOCKING
            if trimmed.contains("async fn ")
                || trimmed.contains("async move")
                || trimmed.contains("async {")
                || trimmed.contains("async function ")
                || trimmed.starts_with("#[tokio::")
            {
                in_async_scope = true;
            } else if (trimmed.starts_with("pub fn ") || trimmed.starts_with("fn "))
                && !trimmed.contains("async ")
            {
                in_async_scope = false;
            }

            for (rule, regex) in self.built_in_rules.iter().chain(compiled_custom.iter()) {
                if rule.id == "INV-ARCH-DAL-LEAK" && !is_frontend_path {
                    continue; // Skip DAL leak check on backend files
                }

                if rule.id == "INV-REL-ASYNC-BLOCKING"
                    && line.contains("std::thread::sleep")
                    && !in_async_scope
                {
                    continue; // Legitimate synchronous sleep inside non-async function
                }

                if rule.id == "INV-SEC-HARDCODED-SECRET" {
                    let upper = trimmed.to_ascii_uppercase();
                    let is_placeholder_or_header = upper.contains("_HEADER")
                        || upper.contains("YOUR_API_KEY")
                        || upper.contains("REPLACE_ME")
                        || upper.contains("X-INTERNAL-");
                    if is_placeholder_or_header && !trimmed.contains("AKIA") && !trimmed.contains("ghp_") && !trimmed.contains("sk-") && !trimmed.contains("eyJ") {
                        continue;
                    }
                }

                let mut matched = regex.is_match(line);
                if !matched
                    && rule.id == "INV-SEC-RAW-SQL"
                    && idx >= sql_window_skip_until
                    && (trimmed.contains("format!(") || trimmed.contains("fmt.Sprintf("))
                {
                    let end_idx = (idx + 2).min(lines.len().saturating_sub(1));
                    let joined = lines[idx..=end_idx].join(" ");
                    if regex.is_match(&joined) {
                        matched = true;
                        sql_window_skip_until = end_idx + 1;
                    }
                }

                if matched {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multilang_vulnerabilities_and_hallucinated_apis() {
        let checker = CodeInvariantChecker::new();
        assert_eq!(checker.built_in_rules.len(), 12);

        let polyglot_code = r#"
// Rust SQLi + Hallucinated Option method + TLS bypass
let q = format!("SELECT * FROM users WHERE id = {}", user_id);
if opt.is_none_or_empty() { return; }
let client = reqwest::Client::builder().danger_accept_invalid_certs(true).build();
// C++ buffer overflow
strcpy(dest_buf, untrusted_input);
// TypeScript hallucinated JSON method + React XSS
const parsed = JSON.tryParse(raw);
return <div dangerouslySetInnerHTML={{ __html: raw }} />;
// Python unsafe deserialization + hallucinated asyncio method
obj = pickle.loads(payload)
await asyncio.sleep_ms(100)
// Go SQLi + deprecated ioutil
query := fmt.Sprintf("SELECT * FROM accounts WHERE name = '%s'", name)
data, _ := ioutil.ReadFile(path)
// PowerShell injection
Invoke-Expression $userCmd
"#;
        let report = checker.audit_code("src/polyglot_audit.rs", polyglot_code, None);
        assert_eq!(report.verdict, AuditVerdict::Block);
        let rule_ids: Vec<&str> = report.violations.iter().map(|v| v.rule_id.as_str()).collect();
        assert!(rule_ids.contains(&"INV-SEC-RAW-SQL"));
        assert!(rule_ids.contains(&"INV-TEMP-HALLUCINATED-API"));
        assert!(rule_ids.contains(&"INV-SEC-WEAK-CRYPTO-TLS"));
        assert!(rule_ids.contains(&"INV-SEC-MEMORY-UNSAFE"));
        assert!(rule_ids.contains(&"INV-SEC-EVAL-EXEC"));
        assert!(rule_ids.contains(&"INV-SEC-PATH-TRAVERSAL"));
        assert!(rule_ids.contains(&"INV-TEMP-DEPRECATED-LIB"));
    }
}
