use crate::grounding::VerificationReport;
use crate::memory::ProjectDossier;
use crate::research::SourceChunk;
use crate::temporal::HorizonAnalysis;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TemporalCheckRequest {
    pub model_id: String,
    pub query: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub max_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub response_text: String,
    pub sources: Option<Vec<SourceChunk>>,
}

#[derive(Debug, Deserialize)]
pub struct SaveEntityRequest {
    pub project_id: String,
    pub entity_name: String,
    pub entity_type: String,
    pub definition: String,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EpistemicChatRequest {
    pub project_id: String,
    pub model_id: String,
    pub user_query: String,
    pub force_research: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct EpistemicChatResponse {
    pub project_id: String,
    pub model_id: String,
    pub answer: String,
    pub temporal_analysis: HorizonAnalysis,
    pub grounding_sources: Vec<SourceChunk>,
    pub verification_report: VerificationReport,
    pub project_dossier: ProjectDossier,
    pub security_shields_active: bool,
}

#[derive(Debug, Serialize)]
pub struct SecurityAuditReport {
    pub status: String,
    pub ssrf_firewall_enabled: bool,
    pub prompt_injection_shield_enabled: bool,
    pub sql_parameterization_enforced: bool,
    pub memory_isolation_active: bool,
    pub timestamp: String,
    pub total_audited_vectors: usize,
}
