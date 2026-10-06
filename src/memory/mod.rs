pub mod dossier;
pub mod l1_buffer;
pub mod l2_ledger;
pub mod l3_graph;

pub use dossier::{DossierCompiler, ProjectDossier};
pub use l1_buffer::{ChatMessage, WorkingBuffer};
pub use l2_ledger::{DriftEvent, EpisodicLedger, SessionEvent, SessionSummary};
pub use l3_graph::{EntityRelation, ProjectEntity, ProjectGraphView, SemanticEntityGraph};

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MemoryEngine {
    conn: Arc<Mutex<Connection>>,
}

impl MemoryEngine {
    pub fn new_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let _ = conn.execute_batch("PRAGMA foreign_keys = ON;");
        EpisodicLedger::init_tables(&conn)?;
        SemanticEntityGraph::init_tables(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let _ = conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys = ON;");
        EpisodicLedger::init_tables(&conn)?;
        SemanticEntityGraph::init_tables(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn get_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn record_session(&self, session: &SessionSummary) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn();
        EpisodicLedger::record_session(&conn, session)
    }

    pub fn record_event(
        &self,
        session_id: &str,
        event_type: &str,
        description: &str,
        diff: Option<&str>,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.get_conn();
        EpisodicLedger::record_event(&conn, session_id, event_type, description, diff)
    }

    pub fn upsert_entity(&self, entity: &ProjectEntity) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn();
        SemanticEntityGraph::upsert_entity(&conn, entity)
    }

    pub fn link_entities(&self, source_id: &str, target_id: &str, relation: &str) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn();
        SemanticEntityGraph::link_entities(&conn, source_id, target_id, relation)
    }

    pub fn query_entities(&self, project_id: &str, query: &str) -> Result<Vec<ProjectEntity>, rusqlite::Error> {
        let conn = self.get_conn();
        SemanticEntityGraph::query_entities_by_text(&conn, project_id, query)
    }

    pub fn get_project_graph(&self, project_id: &str) -> Result<ProjectGraphView, rusqlite::Error> {
        let conn = self.get_conn();
        SemanticEntityGraph::get_project_graph(&conn, project_id)
    }

    pub fn get_dossier(&self, project_id: &str) -> Result<ProjectDossier, rusqlite::Error> {
        let conn = self.get_conn();
        DossierCompiler::compile(&conn, project_id, None)
    }

    pub fn get_relevant_dossier(&self, project_id: &str, query: &str) -> Result<ProjectDossier, rusqlite::Error> {
        let conn = self.get_conn();
        DossierCompiler::compile(&conn, project_id, Some(query))
    }

    pub fn record_drift_event(&self, event: &DriftEvent) -> Result<i64, rusqlite::Error> {
        let conn = self.get_conn();
        EpisodicLedger::record_drift_event(&conn, event)
    }

    pub fn get_drift_events(&self, limit: usize) -> Result<Vec<DriftEvent>, rusqlite::Error> {
        let conn = self.get_conn();
        EpisodicLedger::get_recent_drift_events(&conn, limit)
    }

    pub fn record_cost_event(
        &self,
        query: &str,
        pruned_tools: usize,
        tokens_saved: usize,
        cost_saved_usd: f64,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.get_conn();
        EpisodicLedger::record_cost_event(&conn, query, pruned_tools, tokens_saved, cost_saved_usd)
    }

    pub fn get_cost_totals(&self) -> Result<(u64, u64, u64, f64), rusqlite::Error> {
        let conn = self.get_conn();
        EpisodicLedger::get_cost_totals(&conn)
    }
}
