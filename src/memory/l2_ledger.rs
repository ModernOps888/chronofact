use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    pub id: i64,
    pub session_id: String,
    pub timestamp: String,
    pub event_type: String, // e.g. "DECISION", "CODE_CHANGE", "ARCHITECTURE"
    pub description: String,
    pub context_diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub project_id: String,
    pub created_at: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEvent {
    pub id: i64,
    pub timestamp: String,
    pub project_id: String,
    pub model_id: String,
    pub query: String,
    pub days_post_freeze: i64,
    pub temporal_risk_score: f32,
    pub outdated_topics_caught: String,
    pub intervention_type: String,
    pub ground_truth_retrieved: String,
}

pub struct EpisodicLedger;

impl EpisodicLedger {
    pub fn init_tables(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                summary TEXT NOT NULL
            );",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                event_type TEXT NOT NULL,
                description TEXT NOT NULL,
                context_diff TEXT,
                FOREIGN KEY(session_id) REFERENCES sessions(session_id)
            );",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS drift_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                project_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                query TEXT NOT NULL,
                days_post_freeze INTEGER NOT NULL,
                temporal_risk_score REAL NOT NULL,
                outdated_topics_caught TEXT NOT NULL,
                intervention_type TEXT NOT NULL,
                ground_truth_retrieved TEXT NOT NULL
            );",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cost_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                query TEXT NOT NULL,
                pruned_tools INTEGER NOT NULL,
                tokens_saved INTEGER NOT NULL,
                cost_saved_usd REAL NOT NULL
            );",
            [],
        )?;

        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_session_events_session ON session_events(session_id);
             CREATE INDEX IF NOT EXISTS idx_drift_events_project ON drift_events(project_id);
             CREATE INDEX IF NOT EXISTS idx_cost_events_ts ON cost_events(timestamp);",
        )?;

        Ok(())
    }

    pub fn record_session(conn: &Connection, session: &SessionSummary) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO sessions (session_id, project_id, created_at, summary)
             VALUES (?1, ?2, ?3, ?4);",
            params![session.session_id, session.project_id, session.created_at, session.summary],
        )?;
        Ok(())
    }

    pub fn record_event(
        conn: &Connection,
        session_id: &str,
        event_type: &str,
        description: &str,
        context_diff: Option<&str>,
    ) -> Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        // Auto-create parent session row so that INNER JOIN in get_recent_events_for_project resolves
        conn.execute(
            "INSERT OR IGNORE INTO sessions (session_id, project_id, created_at, summary)
             VALUES (?1, 'antigravity-ide', ?2, 'Auto-registered session');",
            params![session_id, now],
        )?;
        conn.execute(
            "INSERT INTO session_events (session_id, timestamp, event_type, description, context_diff)
             VALUES (?1, ?2, ?3, ?4, ?5);",
            params![session_id, now, event_type, description, context_diff],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn record_drift_event(conn: &Connection, event: &DriftEvent) -> Result<i64> {
        conn.execute(
            "INSERT INTO drift_events 
             (timestamp, project_id, model_id, query, days_post_freeze, temporal_risk_score, outdated_topics_caught, intervention_type, ground_truth_retrieved)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
            params![
                event.timestamp,
                event.project_id,
                event.model_id,
                event.query,
                event.days_post_freeze,
                event.temporal_risk_score,
                event.outdated_topics_caught,
                event.intervention_type,
                event.ground_truth_retrieved
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_recent_drift_events(conn: &Connection, limit: usize) -> Result<Vec<DriftEvent>> {
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, project_id, model_id, query, days_post_freeze, temporal_risk_score, outdated_topics_caught, intervention_type, ground_truth_retrieved
             FROM drift_events
             ORDER BY id DESC
             LIMIT ?1;",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(DriftEvent {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                project_id: row.get(2)?,
                model_id: row.get(3)?,
                query: row.get(4)?,
                days_post_freeze: row.get(5)?,
                temporal_risk_score: row.get(6)?,
                outdated_topics_caught: row.get(7)?,
                intervention_type: row.get(8)?,
                ground_truth_retrieved: row.get(9)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_recent_events_for_project(conn: &Connection, project_id: &str, limit: usize) -> Result<Vec<SessionEvent>> {
        let mut stmt = conn.prepare(
            "SELECT e.id, e.session_id, e.timestamp, e.event_type, e.description, e.context_diff
             FROM session_events e
             JOIN sessions s ON e.session_id = s.session_id
             WHERE s.project_id = ?1
             ORDER BY e.id DESC
             LIMIT ?2;",
        )?;

        let rows = stmt.query_map(params![project_id, limit as i64], |row| {
            Ok(SessionEvent {
                id: row.get(0)?,
                session_id: row.get(1)?,
                timestamp: row.get(2)?,
                event_type: row.get(3)?,
                description: row.get(4)?,
                context_diff: row.get(5)?,
            })
        })?;

        let mut events = Vec::new();
        for r in rows {
            events.push(r?);
        }
        Ok(events)
    }

    pub fn record_cost_event(
        conn: &Connection,
        query: &str,
        pruned_tools: usize,
        tokens_saved: usize,
        cost_saved_usd: f64,
    ) -> Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO cost_events (timestamp, query, pruned_tools, tokens_saved, cost_saved_usd)
             VALUES (?1, ?2, ?3, ?4, ?5);",
            params![now, query, pruned_tools as i64, tokens_saved as i64, cost_saved_usd],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_cost_totals(conn: &Connection) -> Result<(u64, u64, u64, f64)> {
        let mut stmt = conn.prepare(
            "SELECT COUNT(*), COALESCE(SUM(pruned_tools), 0), COALESCE(SUM(tokens_saved), 0), COALESCE(SUM(cost_saved_usd), 0.0) FROM cost_events;"
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let count: i64 = row.get(0)?;
            let pruned: i64 = row.get(1)?;
            let tokens: i64 = row.get(2)?;
            let cost: f64 = row.get(3)?;
            Ok((count as u64, pruned as u64, tokens as u64, cost))
        } else {
            Ok((0, 0, 0, 0.0))
        }
    }
}
