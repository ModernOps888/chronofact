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
        let now = chrono::Utc::now().to_rfc3339();

        // 1. Fetch all L3 entities for the project
        let all_entities = SemanticEntityGraph::query_entities_by_text(conn, project_id, "")?;

        // 2. Relevance filtering if query is provided
        let entities: Vec<super::l3_graph::ProjectEntity> = if let Some(q) = query {
            let q_trimmed = q.trim();
            if q_trimmed.is_empty() {
                Vec::new()
            } else {
                let stopwords = [
                    "the", "is", "at", "which", "on", "and", "a", "an", "in", "to", "for", "with", "as",
                    "by", "that", "this", "it", "are", "be", "or", "from", "how", "what", "can", "you",
                    "tell", "me", "about", "please", "we", "do", "i", "my", "our", "so", "now", "like",
                ];
                let tokens: Vec<String> = q_trimmed
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| w.len() > 2 && !stopwords.contains(w))
                    .map(|s| s.to_string())
                    .collect();

                if tokens.is_empty() {
                    Vec::new()
                } else {
                    all_entities
                        .into_iter()
                        .filter(|e| {
                            let name_lower = e.entity_name.to_lowercase();
                            let def_lower = e.definition.to_lowercase();
                            let name_words: Vec<&str> = name_lower
                                .split(|c: char| !c.is_alphanumeric())
                                .filter(|w| w.len() > 1)
                                .collect();
                            let def_words: Vec<&str> = def_lower
                                .split(|c: char| !c.is_alphanumeric())
                                .filter(|w| w.len() > 2)
                                .collect();
                            tokens.iter().any(|t| name_words.contains(&t.as_str()) || def_words.contains(&t.as_str()))
                        })
                        .collect()
                }
            }
        } else {
            all_entities
        };

        // Zero-Pollution Guard: If a query was explicitly provided and NO relevant entities matched,
        // return an empty dossier so we NEVER pollute new chats with irrelevant memory!
        if query.is_some() && entities.is_empty() {
            return Ok(ProjectDossier {
                project_id: project_id.to_string(),
                generated_at: now,
                active_tech_stack: Vec::new(),
                architectural_invariants: Vec::new(),
                recent_decisions: Vec::new(),
                dossier_markdown: String::new(),
            });
        }

        let mut active_tech_stack = Vec::new();
        let mut architectural_invariants = Vec::new();

        for e in &entities {
            match e.entity_type.as_str() {
                "TECH_STACK" | "DEPENDENCY" => {
                    active_tech_stack.push(format!("{} (v{}) - {}", e.entity_name, e.version, e.definition));
                }
                "ARCHITECTURE_RULE" | "API_CONTRACT" | "DATABASE_SCHEMA" => {
                    architectural_invariants.push(format!("[{}] {}: {}", e.entity_type, e.entity_name, e.definition));
                }
                _ => {}
            }
        }

        // 3. Fetch L2 recent events only if no query filter is active (prevents chat history dumping)
        let recent_events = if query.is_some() {
            Vec::new()
        } else {
            EpisodicLedger::get_recent_events_for_project(conn, project_id, 10)?
        };
        let mut recent_decisions = Vec::new();

        for ev in &recent_events {
            recent_decisions.push(format!("(Session {}) [{}]: {}", ev.session_id, ev.event_type, ev.description));
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

        md.push_str(&format!("\n<!-- ChronoFact Memory Snapshot: {} -->\n</chronofact_project_truth_dossier>\n", now));

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
