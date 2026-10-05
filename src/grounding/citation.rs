use super::verifier::VerificationReport;
use crate::research::SourceChunk;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundedCitation {
    pub source_id: String,
    pub title: String,
    pub url: String,
    pub verified_claims_count: usize,
}

pub struct CitationMapper;

impl CitationMapper {
    pub fn build_citations(report: &VerificationReport, sources: &[SourceChunk]) -> Vec<GroundedCitation> {
        let mut citations = Vec::new();

        for source in sources {
            let count = report.claims.iter()
                .filter(|c| c.matched_source_ids.contains(&source.id))
                .count();

            citations.push(GroundedCitation {
                source_id: source.id.clone(),
                title: source.title.clone(),
                url: source.url.clone(),
                verified_claims_count: count,
            });
        }

        citations
    }

    pub fn format_markdown_footer(citations: &[GroundedCitation]) -> String {
        if citations.is_empty() {
            return String::new();
        }

        let mut out = String::from("\n\n---\n### 🔍 Verified Grounded Sources\n");
        for cit in citations {
            out.push_str(&format!("* **[{}]** [{}]({}) (Supports {} claims)\n",
                cit.source_id,
                cit.title,
                cit.url,
                cit.verified_claims_count
            ));
        }
        out
    }
}
