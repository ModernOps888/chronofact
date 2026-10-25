use super::calibrator::{EmbeddingProvider, MemoryCalibrator};
use super::l2_ledger::EpisodicLedger;
use super::l3_graph::SemanticEntityGraph;
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDossier {
    pub project_id: String,
    pub generated_at: String,
    pub active_tech_stack: Vec<String>,
    pub architectural_invariants: Vec<String>,
    pub recent_decisions: Vec<String>,
    pub dossier_markdown: String,
}

pub struct DossierCompiler;

impl DossierCompiler {
    pub fn compile(conn: &Connection, project_id: &str, query: Option<&str>) -> Result<ProjectDossier> {
        let now_dt = chrono::Utc::now();
        let now = now_dt.to_rfc3339();

        // 1. Fetch all L3 entities for the project
        let all_entities = SemanticEntityGraph::query_entities_by_text(conn, project_id, "")?;

        let stopwords = [
            "the", "is", "at", "which", "on", "and", "a", "an", "in", "to", "for", "with", "as",
            "by", "that", "this", "it", "are", "be", "or", "from", "how", "what", "can", "you",
            "tell", "me", "about", "please", "we", "do", "i", "my", "our", "so", "now", "like",
        ];

        let query_tokens: Vec<String> = query
            .map(|q| {
                q.trim()
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| w.len() >= 2 && !stopwords.contains(w))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // 2. Relevance filtering + MemoryCalibrator + temporal recency decay if query is provided
        let entities: Vec<super::l3_graph::ProjectEntity> = if query.is_some() {
            if query_tokens.is_empty() {
                Vec::new()
            } else {
                let mut scored_candidates: Vec<(super::l3_graph::ProjectEntity, f32)> = Vec::new();
                for e in all_entities {
                    let name_lower = e.entity_name.to_lowercase();
                    let def_lower = e.definition.to_lowercase();
                    let name_words: Vec<&str> = name_lower
                        .split(|c: char| !c.is_alphanumeric())
                        .filter(|w| w.len() >= 2)
                        .collect();
                    let def_words: Vec<&str> = def_lower
                        .split(|c: char| !c.is_alphanumeric())
                        .filter(|w| w.len() >= 2)
                        .collect();
                    let name_compact: String = name_lower.chars().filter(|c| c.is_alphanumeric()).collect();

                    let mut matched_weight = 0.0_f32;
                    for t in &query_tokens {
                        let t_str = t.as_str();
                        if name_words.contains(&t_str) || (t_str.len() >= 4 && name_compact.contains(t_str)) {
                            matched_weight += 2.0;
                        } else if def_words.contains(&t_str) {
                            matched_weight += 1.0;
                        }
                    }

                    if matched_weight > 0.0 {
                        let raw_lexical = (matched_weight / (query_tokens.len() as f32).max(1.0)).min(1.0);

                        // Exponential temporal recency decay: exp(-0.002 * age_days), bounded in [0.85, 1.0]
                        let age_days = chrono::DateTime::parse_from_rfc3339(&e.updated_at)
                            .map(|dt| {
                                (now_dt.signed_duration_since(dt.with_timezone(&chrono::Utc)).num_days() as f32)
                                    .max(0.0)
                            })
                            .unwrap_or(0.0);
                        let recency_weight = (-0.002_f32 * age_days).exp().clamp(0.85, 1.0);
                        let calibrated_score = raw_lexical * recency_weight;
                        scored_candidates.push((e, calibrated_score));
                    }
                }

                MemoryCalibrator::filter_adaptive(
                    scored_candidates,
                    EmbeddingProvider::LexicalTokenOverlap,
                    0.25,
                    15,
                )
                .into_iter()
                .map(|(e, _)| e)
                .collect()
            }
        } else {
            all_entities
        };

        let mut active_tech_stack = Vec::new();
        let mut architectural_invariants = Vec::new();

        for e in &entities {
            let norm_type = e.entity_type.trim().to_uppercase();
            match norm_type.as_str() {
                "TECH_STACK" | "DEPENDENCY" | "FRAMEWORK" | "RUNTIME" | "LIBRARY" => {
                    active_tech_stack.push(format!("{} (v{}) - {}", e.entity_name, e.version, e.definition));
                }
                "ARCHITECTURE_RULE" | "API_CONTRACT" | "DATABASE_SCHEMA" | "DATA_MODEL" | "INVARIANT" | "SECURITY_RULE" => {
                    architectural_invariants.push(format!("[{}] {}: {}", norm_type, e.entity_name, e.definition));
                }
                other if !other.is_empty() => {
                    architectural_invariants.push(format!("[{}] {}: {}", other, e.entity_name, e.definition));
                }
                _ => {}
            }
        }

        // 3. Fetch L2 recent events: if query is provided, filter to only query-relevant decisions
        let recent_events = if query.is_some() {
            if query_tokens.is_empty() {
                Vec::new()
            } else {
                let candidate_events = EpisodicLedger::get_recent_events_for_project(conn, project_id, 25)?;
                candidate_events
                    .into_iter()
                    .filter(|ev| {
                        let desc_lower = ev.description.to_lowercase();
                        let desc_words: Vec<&str> = desc_lower
                            .split(|c: char| !c.is_alphanumeric())
                            .filter(|w| w.len() > 2)
                            .collect();
                        query_tokens.iter().any(|t| desc_words.contains(&t.as_str()))
                    })
                    .take(5)
                    .collect()
            }
        } else {
            EpisodicLedger::get_recent_events_for_project(conn, project_id, 10)?
        };

        let mut recent_decisions = Vec::new();
        for ev in &recent_events {
            recent_decisions.push(format!("(Session {}) [{}]: {}", ev.session_id, ev.event_type, ev.description));
        }

        // Zero-Pollution & Empty-Shell Guard: If a query was explicitly provided and NO relevant
        // tech stack, invariants, or session decisions matched, return an empty dossier string!
        if query.is_some()
            && active_tech_stack.is_empty()
            && architectural_invariants.is_empty()
            && recent_decisions.is_empty()
        {
            return Ok(ProjectDossier {
                project_id: project_id.to_string(),
                generated_at: now,
                active_tech_stack,
                architectural_invariants,
                recent_decisions,
                dossier_markdown: String::new(),
            });
        }

        // 4. Assemble Markdown Dossier (Header without dynamic timestamp for 100% KV cache prefix hits)
        let mut md = format!(
            "<chronofact_project_truth_dossier project=\"{}\">\n\
            ### 📌 ChronoFact Persistent Project Memory (Cross-Session Truth)\n\n",
            project_id
        );

        if !active_tech_stack.is_empty() {
            md.push_str("#### ⚙️ Active Tech Stack & Invariants\n");
            for item in &active_tech_stack {
                md.push_str(&format!("* {}\n", item));
            }
        }

        if !architectural_invariants.is_empty() {
            md.push_str("\n#### 🏛️ Architecture Rules & Contracts\n");
            for item in &architectural_invariants {
                md.push_str(&format!("* {}\n", item));
            }
        }

        if !recent_decisions.is_empty() {
            md.push_str("\n#### 📜 Recent Session Decisions (Past Chats)\n");
            for dec in &recent_decisions {
                md.push_str(&format!("* {}\n", dec));
            }
        }

        md.push_str("\n<!-- ChronoFact Memory Snapshot: deterministic-kv-prefix -->\n</chronofact_project_truth_dossier>\n");

        Ok(ProjectDossier {
            project_id: project_id.to_string(),
            generated_at: now,
            active_tech_stack,
            architectural_invariants,
            recent_decisions,
            dossier_markdown: md,
        })
    }
}
