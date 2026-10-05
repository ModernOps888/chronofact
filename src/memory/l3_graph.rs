use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntity {
    pub id: String,
    pub project_id: String,
    pub entity_name: String,
    pub entity_type: String, // "TECH_STACK", "API_CONTRACT", "ARCHITECTURE_RULE", "DATABASE_SCHEMA", "DATA_MODEL"
    pub definition: String,
    pub version: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRelation {
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String, // "DEPENDS_ON", "IMPLEMENTS", "CONSTRAINS"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGraphView {
    pub entities: Vec<ProjectEntity>,
    pub relations: Vec<EntityRelation>,
}

pub struct SemanticEntityGraph;

impl SemanticEntityGraph {
    pub fn init_tables(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS project_entities (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                entity_name TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                definition TEXT NOT NULL,
                version TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS entity_relations (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                PRIMARY KEY (source_id, target_id, relation_type),
                FOREIGN KEY(source_id) REFERENCES project_entities(id),
                FOREIGN KEY(target_id) REFERENCES project_entities(id)
            );",
            [],
        )?;

        // Index on project_id and entity_name
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_entities_project ON project_entities(project_id);",
            [],
        )?;

        Ok(())
    }

    pub fn upsert_entity(conn: &Connection, entity: &ProjectEntity) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO project_entities 
             (id, project_id, entity_name, entity_type, definition, version, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
            params![
                entity.id,
                entity.project_id,
                entity.entity_name,
                entity.entity_type,
                entity.definition,
                entity.version,
                entity.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn link_entities(
        conn: &Connection,
        source_id: &str,
        target_id: &str,
        relation_type: &str,
    ) -> Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO entity_relations (source_id, target_id, relation_type)
             VALUES (?1, ?2, ?3);",
            params![source_id, target_id, relation_type],
        )?;
        Ok(())
    }

    pub fn query_entities_by_text(
        conn: &Connection,
        project_id: &str,
        query: &str,
    ) -> Result<Vec<ProjectEntity>> {
        let pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, project_id, entity_name, entity_type, definition, version, updated_at
             FROM project_entities
             WHERE project_id = ?1 AND (entity_name LIKE ?2 OR definition LIKE ?2)
             ORDER BY updated_at DESC;",
        )?;

        let rows = stmt.query_map(params![project_id, pattern], |row| {
            Ok(ProjectEntity {
                id: row.get(0)?,
                project_id: row.get(1)?,
                entity_name: row.get(2)?,
                entity_type: row.get(3)?,
                definition: row.get(4)?,
                version: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        let mut entities = Vec::new();
        for r in rows {
            entities.push(r?);
        }
        Ok(entities)
    }

    pub fn get_project_graph(conn: &Connection, project_id: &str) -> Result<ProjectGraphView> {
        let mut stmt_entities = conn.prepare(
            "SELECT id, project_id, entity_name, entity_type, definition, version, updated_at
             FROM project_entities
             WHERE project_id = ?1;",
        )?;

        let entities_rows = stmt_entities.query_map(params![project_id], |row| {
            Ok(ProjectEntity {
                id: row.get(0)?,
                project_id: row.get(1)?,
                entity_name: row.get(2)?,
                entity_type: row.get(3)?,
                definition: row.get(4)?,
                version: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        let mut entities = Vec::new();
        for r in entities_rows {
            entities.push(r?);
        }

        let mut stmt_rel = conn.prepare(
            "SELECT r.source_id, r.target_id, r.relation_type
             FROM entity_relations r
             JOIN project_entities e ON r.source_id = e.id
             WHERE e.project_id = ?1;",
        )?;

        let rel_rows = stmt_rel.query_map(params![project_id], |row| {
            Ok(EntityRelation {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                relation_type: row.get(2)?,
            })
        })?;

        let mut relations = Vec::new();
        for r in rel_rows {
            relations.push(r?);
        }

        Ok(ProjectGraphView { entities, relations })
    }
}
