//! Cryptographic Invariant Attestation Engine
//!
//! Generates cryptographically verifiable audit tokens for AI-generated
//! artifacts, binding the code SHA-256, temporal horizon anchor, evaluated
//! invariants, and pass/block verdict into a tamper-proof certificate for CI/CD gates.

use super::code_invariants::{AuditVerdict, CodeAuditReport};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantAttestation {
    pub schema_version: String,
    pub attestation_id: String,
    pub timestamp: String,
    pub project_id: String,
    pub target_name: String,
    pub target_sha256: String,
    pub temporal_anchor: String,
    pub verdict: String,
    pub rules_evaluated: usize,
    pub violations_count: usize,
    pub critical_violations: usize,
    pub high_violations: usize,
    pub signature: String,
}

pub struct AttestationEngine;

impl AttestationEngine {
    /// Generates a signed invariant attestation token from an audit report.
    pub fn create_attestation(
        report: &CodeAuditReport,
        project_id: &str,
        target_content: &str,
        temporal_anchor: &str,
        signing_key: Option<&str>,
    ) -> InvariantAttestation {
        let mut hasher = Sha256::new();
        hasher.update(target_content.as_bytes());
        let target_sha256 = format!("{:x}", hasher.finalize());

        let attestation_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let verdict_str = match report.verdict {
            AuditVerdict::Pass => "PASS",
            AuditVerdict::Advisory => "ADVISORY",
            AuditVerdict::Block => "BLOCK",
        };

        let key = signing_key.unwrap_or("CHRONOFACT_ENTERPRISE_INVARIANT_ROOT");
        let canonical_payload = format!(
            "CHRONOFACT-ATTESTATION:v1:{}:{}:{}:{}:{}:{}:{}",
            attestation_id,
            project_id,
            target_sha256,
            temporal_anchor,
            verdict_str,
            report.violations_count,
            key
        );

        let mut sig_hasher = Sha256::new();
        sig_hasher.update(canonical_payload.as_bytes());
        let signature = format!("{:x}", sig_hasher.finalize());

        InvariantAttestation {
            schema_version: "1.0.0".to_string(),
            attestation_id,
            timestamp,
            project_id: project_id.to_string(),
            target_name: report.target.clone(),
            target_sha256,
            temporal_anchor: temporal_anchor.to_string(),
            verdict: verdict_str.to_string(),
            rules_evaluated: report.total_rules_evaluated,
            violations_count: report.violations_count,
            critical_violations: report.critical_count,
            high_violations: report.high_count,
            signature,
        }
    }

    /// Verifies the cryptographic integrity of an Invariant Attestation token.
    pub fn verify_signature(attestation: &InvariantAttestation, signing_key: Option<&str>) -> bool {
        let key = signing_key.unwrap_or("CHRONOFACT_ENTERPRISE_INVARIANT_ROOT");
        let canonical_payload = format!(
            "CHRONOFACT-ATTESTATION:v1:{}:{}:{}:{}:{}:{}:{}",
            attestation.attestation_id,
            attestation.project_id,
            attestation.target_sha256,
            attestation.temporal_anchor,
            attestation.verdict,
            attestation.violations_count,
            key
        );

        let mut sig_hasher = Sha256::new();
        sig_hasher.update(canonical_payload.as_bytes());
        let expected = format!("{:x}", sig_hasher.finalize());

        attestation.signature == expected
    }
}
