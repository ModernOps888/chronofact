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

pub const UNSIGNED_MARKER: &str = "UNSIGNED:CHRONOFACT_ATTESTATION_KEY not configured";

/// HMAC-SHA256 (RFC 2104) on top of sha2; avoids adding a dependency.
pub fn hmac_sha256_hex(key: &[u8], msg: &[u8]) -> String {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let d = Sha256::digest(key);
        k[..32].copy_from_slice(&d);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    format!("{:x}", outer.finalize())
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

        let canonical_payload = format!(
            "CHRONOFACT-ATTESTATION:v1:{}:{}:{}:{}:{}:{}",
            attestation_id, project_id, target_sha256, temporal_anchor, verdict_str, report.violations_count
        );
        // HMAC-SHA256 with an operator-supplied key. With no key the token is explicitly
        // unsigned rather than "signed" with a publicly known literal.
        let signature = match Self::resolve_key(signing_key) {
            Some(key) => hmac_sha256_hex(key.as_bytes(), canonical_payload.as_bytes()),
            None => UNSIGNED_MARKER.to_string(),
        };

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
        let key = match Self::resolve_key(signing_key) {
            Some(k) => k,
            None => return false,
        };
        if attestation.signature == UNSIGNED_MARKER {
            return false;
        }
        let canonical_payload = format!(
            "CHRONOFACT-ATTESTATION:v1:{}:{}:{}:{}:{}:{}",
            attestation.attestation_id,
            attestation.project_id,
            attestation.target_sha256,
            attestation.temporal_anchor,
            attestation.verdict,
            attestation.violations_count
        );
        let expected = hmac_sha256_hex(key.as_bytes(), canonical_payload.as_bytes());
        // Constant-time comparison
        let a = attestation.signature.as_bytes();
        let b = expected.as_bytes();
        a.len() == b.len() && a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }

    /// Explicit key, else `CHRONOFACT_ATTESTATION_KEY` env var; empty counts as unset.
    fn resolve_key(signing_key: Option<&str>) -> Option<String> {
        signing_key
            .map(|k| k.to_string())
            .or_else(|| std::env::var("CHRONOFACT_ATTESTATION_KEY").ok())
            .filter(|k| !k.is_empty())
    }
}
