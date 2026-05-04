use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::Config;
use crate::error::YojanaError;
use crate::state;

pub struct Db {
    conn: Mutex<Connection>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub ts: i64,
    pub kind: String,
    pub payload: serde_json::Value,
}

// --- Project types ---

#[derive(Debug)]
pub struct ProjectRow {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub history: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Default)]
pub struct ProjectUpdates {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

// --- Task types ---

#[derive(Debug)]
pub struct TaskRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_slug: String,
    pub sequence_number: i64,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub status: String,
    pub slice_type: Option<String>,
    pub acceptance_criteria: String,
    pub decisions: String,
    pub implementation_plan: Option<String>,
    pub execution_record: Option<String>,
    pub reproduction: Option<String>,
    pub root_cause: Option<String>,
    pub context_refs: String,
    pub files: String,
    pub tags: String,
    pub history: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct CreateTaskParams {
    pub project_id: Uuid,
    pub project_slug: String,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub slice_type: Option<String>,
    pub acceptance_criteria: String,
    pub decisions: String,
    pub context_refs: String,
    pub files: String,
    pub tags: String,
    pub implementation_plan: Option<String>,
    pub execution_record: Option<String>,
    pub reproduction: Option<String>,
    pub root_cause: Option<String>,
}

#[derive(Debug, Default)]
pub struct TaskUpdates {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub slice_type: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub decisions: Option<String>,
    pub implementation_plan: Option<String>,
    pub execution_record: Option<String>,
    pub reproduction: Option<String>,
    pub root_cause: Option<String>,
    pub context_refs: Option<String>,
    pub files: Option<String>,
    pub tags: Option<String>,
}

enum TaskIdentifier {
    Uuid(Uuid),
    SlugSeq(String, i64),
}

// --- Edge types ---

pub const VALID_EDGE_TYPES: &[&str] =
    &["depends_on", "relates_to", "supersedes", "refines", "motivated_by"];

#[derive(Debug)]
pub struct EdgeRow {
    pub id: Uuid,
    pub source_task_id: Uuid,
    pub target_task_id: Uuid,
    pub edge_type: String,
    pub note: Option<String>,
    pub created_at: i64,
}

// --- Project helpers ---

fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRow> {
    let id_bytes: Vec<u8> = row.get("id")?;
    let id = Uuid::from_slice(&id_bytes).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e))
    })?;
    Ok(ProjectRow {
        id,
        slug: row.get("slug")?,
        title: row.get("title")?,
        description: row.get("description")?,
        status: row.get("status")?,
        history: row.get("history")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn get_by_id(conn: &Connection, id: &Uuid) -> Result<Option<ProjectRow>, YojanaError> {
    let mut stmt = conn.prepare("SELECT * FROM projects WHERE id = ?1")?;
    let row = stmt
        .query_row(rusqlite::params![id.as_bytes().as_slice()], map_project_row)
        .optional()?;
    Ok(row)
}

fn get_by_slug(conn: &Connection, slug: &str) -> Result<Option<ProjectRow>, YojanaError> {
    let mut stmt = conn.prepare("SELECT * FROM projects WHERE slug = ?1")?;
    let row = stmt
        .query_row(rusqlite::params![slug], map_project_row)
        .optional()?;
    Ok(row)
}

fn resolve_project(
    conn: &Connection,
    id: Option<&str>,
    slug: Option<&str>,
) -> Result<ProjectRow, YojanaError> {
    if let Some(id_str) = id {
        let uuid = Uuid::parse_str(id_str)
            .map_err(|_| YojanaError::InvalidInput(format!("invalid UUID: {id_str}")))?;
        return get_by_id(conn, &uuid)?
            .ok_or_else(|| YojanaError::NotFound(format!("project id '{id_str}'")));
    }
    if let Some(slug) = slug {
        return get_by_slug(conn, slug)?
            .ok_or_else(|| YojanaError::NotFound(format!("project slug '{slug}'")));
    }
    Err(YojanaError::InvalidInput("id or slug required".into()))
}

// --- Task helpers ---

const TASK_SELECT: &str = "\
    SELECT t.id, t.project_id, p.slug AS project_slug, t.sequence_number, \
    t.title, t.description, t.category, t.status, t.slice_type, \
    t.acceptance_criteria, t.decisions, t.implementation_plan, \
    t.execution_record, t.reproduction, t.root_cause, \
    t.context_refs, t.files, t.tags, t.history, t.created_at, t.updated_at \
    FROM tasks t JOIN projects p ON t.project_id = p.id";

fn uuid_from_blob(row: &rusqlite::Row<'_>, col: &str) -> rusqlite::Result<Uuid> {
    let bytes: Vec<u8> = row.get(col)?;
    Uuid::from_slice(&bytes).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e))
    })
}

fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: uuid_from_blob(row, "id")?,
        project_id: uuid_from_blob(row, "project_id")?,
        project_slug: row.get("project_slug")?,
        sequence_number: row.get("sequence_number")?,
        title: row.get("title")?,
        description: row.get("description")?,
        category: row.get("category")?,
        status: row.get("status")?,
        slice_type: row.get("slice_type")?,
        acceptance_criteria: row.get("acceptance_criteria")?,
        decisions: row.get("decisions")?,
        implementation_plan: row.get("implementation_plan")?,
        execution_record: row.get("execution_record")?,
        reproduction: row.get("reproduction")?,
        root_cause: row.get("root_cause")?,
        context_refs: row.get("context_refs")?,
        files: row.get("files")?,
        tags: row.get("tags")?,
        history: row.get("history")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn get_task_by_uuid(conn: &Connection, id: &Uuid) -> Result<Option<TaskRow>, YojanaError> {
    let sql = format!("{TASK_SELECT} WHERE t.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let row = stmt
        .query_row(rusqlite::params![id.as_bytes().as_slice()], map_task_row)
        .optional()?;
    Ok(row)
}

fn get_task_by_slug_seq(
    conn: &Connection,
    slug: &str,
    seq: i64,
) -> Result<Option<TaskRow>, YojanaError> {
    let sql = format!("{TASK_SELECT} WHERE p.slug = ?1 AND t.sequence_number = ?2");
    let mut stmt = conn.prepare(&sql)?;
    let row = stmt
        .query_row(rusqlite::params![slug, seq], map_task_row)
        .optional()?;
    Ok(row)
}

fn parse_task_identifier(s: &str) -> Result<TaskIdentifier, YojanaError> {
    if let Ok(uuid) = Uuid::parse_str(s) {
        return Ok(TaskIdentifier::Uuid(uuid));
    }
    if let Some((slug, num_str)) = s.rsplit_once('/') {
        if let Ok(num) = num_str.parse::<i64>() {
            return Ok(TaskIdentifier::SlugSeq(slug.to_string(), num));
        }
    }
    Err(YojanaError::InvalidInput(format!(
        "invalid task identifier '{s}'; expected UUID or 'project-slug/N'"
    )))
}

fn next_sequence_number(conn: &Connection, project_id: &Uuid) -> Result<i64, YojanaError> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM tasks WHERE project_id = ?1",
    )?;
    let seq: i64 = stmt.query_row(
        rusqlite::params![project_id.as_bytes().as_slice()],
        |row| row.get(0),
    )?;
    Ok(seq)
}

fn resolve_task(conn: &Connection, identifier: &str) -> Result<TaskRow, YojanaError> {
    let row = match parse_task_identifier(identifier)? {
        TaskIdentifier::Uuid(id) => get_task_by_uuid(conn, &id)?,
        TaskIdentifier::SlugSeq(slug, seq) => get_task_by_slug_seq(conn, &slug, seq)?,
    };
    row.ok_or_else(|| YojanaError::NotFound(format!("task '{identifier}'")))
}

use rusqlite::OptionalExtension;

impl Db {
    pub fn open(config: &Config) -> anyhow::Result<Self> {
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&config.db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute_batch(include_str!("../migrations/0001_initial.sql"))?;
        conn.execute_batch(include_str!("../migrations/0002_tasks.sql"))?;
        conn.execute_batch(include_str!("../migrations/0003_edges.sql"))
    }

    // --- Project methods ---

    pub fn create_project(
        &self,
        slug: &str,
        title: &str,
        description: &str,
    ) -> Result<ProjectRow, YojanaError> {
        let conn = self.conn.lock();
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().timestamp_millis();
        let history = serde_json::to_string(&vec![HistoryEntry {
            ts: now,
            kind: "project_created".into(),
            payload: serde_json::json!({}),
        }])?;

        conn.execute(
            "INSERT INTO projects (id, slug, title, description, status, history, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?6)",
            rusqlite::params![id.as_bytes().as_slice(), slug, title, description, history, now],
        )
        .map_err(|e| {
            if let rusqlite::Error::SqliteFailure(ref err, _) = e {
                if err.extended_code == 2067 {
                    return YojanaError::Conflict(format!(
                        "project slug '{slug}' already exists"
                    ));
                }
            }
            YojanaError::Db(e)
        })?;

        get_by_id(&conn, &id)?.ok_or_else(|| YojanaError::NotFound("just-created project".into()))
    }

    pub fn get_project(
        &self,
        id: Option<&str>,
        slug: Option<&str>,
    ) -> Result<Option<ProjectRow>, YojanaError> {
        let conn = self.conn.lock();
        if let Some(id_str) = id {
            let uuid = Uuid::parse_str(id_str)
                .map_err(|_| YojanaError::InvalidInput(format!("invalid UUID: {id_str}")))?;
            return get_by_id(&conn, &uuid);
        }
        if let Some(slug) = slug {
            return get_by_slug(&conn, slug);
        }
        Err(YojanaError::InvalidInput("id or slug required".into()))
    }

    pub fn list_projects(&self, status: Option<&str>) -> Result<Vec<ProjectRow>, YojanaError> {
        let conn = self.conn.lock();
        let rows = if let Some(status) = status {
            let mut stmt =
                conn.prepare("SELECT * FROM projects WHERE status = ?1 ORDER BY created_at")?;
            stmt.query_map(rusqlite::params![status], map_project_row)?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = conn.prepare("SELECT * FROM projects ORDER BY created_at")?;
            stmt.query_map([], map_project_row)?
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    pub fn update_project(
        &self,
        id: Option<&str>,
        slug: Option<&str>,
        updates: ProjectUpdates,
    ) -> Result<ProjectRow, YojanaError> {
        let conn = self.conn.lock();
        let project = resolve_project(&conn, id, slug)?;
        let now = chrono::Utc::now().timestamp_millis();
        let mut history: Vec<HistoryEntry> = serde_json::from_str(&project.history)?;

        if let Some(ref new_title) = updates.title {
            if new_title != &project.title {
                history.push(HistoryEntry {
                    ts: now,
                    kind: "updated".into(),
                    payload: serde_json::json!({"field": "title", "from": project.title, "to": new_title}),
                });
            }
        }
        if let Some(ref new_desc) = updates.description {
            if new_desc != &project.description {
                history.push(HistoryEntry {
                    ts: now,
                    kind: "updated".into(),
                    payload: serde_json::json!({"field": "description"}),
                });
            }
        }
        if let Some(ref new_status) = updates.status {
            if new_status != &project.status {
                history.push(HistoryEntry {
                    ts: now,
                    kind: "status_changed".into(),
                    payload: serde_json::json!({"from": project.status, "to": new_status}),
                });
            }
        }

        let new_title = updates.title.as_deref().unwrap_or(&project.title);
        let new_desc = updates.description.as_deref().unwrap_or(&project.description);
        let new_status = updates.status.as_deref().unwrap_or(&project.status);
        let history_json = serde_json::to_string(&history)?;

        conn.execute(
            "UPDATE projects SET title=?1, description=?2, status=?3, history=?4, updated_at=?5 WHERE id=?6",
            rusqlite::params![new_title, new_desc, new_status, history_json, now, project.id.as_bytes().as_slice()],
        )?;

        get_by_id(&conn, &project.id)?
            .ok_or_else(|| YojanaError::NotFound("updated project".into()))
    }

    // --- Task methods ---

    pub fn create_task(&self, params: CreateTaskParams) -> Result<TaskRow, YojanaError> {
        let conn = self.conn.lock();
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().timestamp_millis();
        let seq = next_sequence_number(&conn, &params.project_id)?;
        let history = serde_json::to_string(&vec![HistoryEntry {
            ts: now,
            kind: "task_created".into(),
            payload: serde_json::json!({"sequence_number": seq, "project": params.project_slug}),
        }])?;

        conn.execute(
            "INSERT INTO tasks (\
                id, project_id, sequence_number, title, description, \
                category, status, slice_type, acceptance_criteria, decisions, \
                implementation_plan, execution_record, reproduction, root_cause, \
                context_refs, files, tags, history, created_at, updated_at\
            ) VALUES (\
                ?1, ?2, ?3, ?4, ?5, \
                ?6, 'needs-triage', ?7, ?8, ?9, \
                ?10, ?11, ?12, ?13, \
                ?14, ?15, ?16, ?17, ?18, ?18\
            )",
            rusqlite::params![
                id.as_bytes().as_slice(),
                params.project_id.as_bytes().as_slice(),
                seq,
                params.title,
                params.description,
                params.category,
                params.slice_type,
                params.acceptance_criteria,
                params.decisions,
                params.implementation_plan,
                params.execution_record,
                params.reproduction,
                params.root_cause,
                params.context_refs,
                params.files,
                params.tags,
                history,
                now,
            ],
        )?;

        get_task_by_uuid(&conn, &id)?
            .ok_or_else(|| YojanaError::NotFound("just-created task".into()))
    }

    pub fn get_task(&self, identifier: &str) -> Result<Option<TaskRow>, YojanaError> {
        let conn = self.conn.lock();
        match parse_task_identifier(identifier)? {
            TaskIdentifier::Uuid(id) => get_task_by_uuid(&conn, &id),
            TaskIdentifier::SlugSeq(slug, seq) => get_task_by_slug_seq(&conn, &slug, seq),
        }
    }

    pub fn update_task(
        &self,
        identifier: &str,
        updates: TaskUpdates,
    ) -> Result<TaskRow, YojanaError> {
        let conn = self.conn.lock();
        let task = resolve_task(&conn, identifier)?;
        let now = chrono::Utc::now().timestamp_millis();
        let mut history: Vec<HistoryEntry> = serde_json::from_str(&task.history)?;

        if let Some(ref new_status) = updates.status {
            if new_status != &task.status {
                state::validate_transition(&task.status, new_status)?;
                history.push(HistoryEntry {
                    ts: now,
                    kind: "status_changed".into(),
                    payload: serde_json::json!({"from": task.status, "to": new_status}),
                });
            }
        }
        if let Some(ref new_title) = updates.title {
            if new_title != &task.title {
                history.push(HistoryEntry {
                    ts: now,
                    kind: "updated".into(),
                    payload: serde_json::json!({"field": "title", "from": task.title, "to": new_title}),
                });
            }
        }

        let new_title = updates.title.as_deref().unwrap_or(&task.title);
        let new_desc = updates.description.as_deref().unwrap_or(&task.description);
        let new_cat = if updates.category.is_some() {
            updates.category.as_deref()
        } else {
            task.category.as_deref()
        };
        let new_status = updates.status.as_deref().unwrap_or(&task.status);
        let new_slice = if updates.slice_type.is_some() {
            updates.slice_type.as_deref()
        } else {
            task.slice_type.as_deref()
        };
        let new_ac = updates
            .acceptance_criteria
            .as_deref()
            .unwrap_or(&task.acceptance_criteria);
        let new_dec = updates.decisions.as_deref().unwrap_or(&task.decisions);
        let new_impl = if updates.implementation_plan.is_some() {
            updates.implementation_plan.as_deref()
        } else {
            task.implementation_plan.as_deref()
        };
        let new_exec = if updates.execution_record.is_some() {
            updates.execution_record.as_deref()
        } else {
            task.execution_record.as_deref()
        };
        let new_repro = if updates.reproduction.is_some() {
            updates.reproduction.as_deref()
        } else {
            task.reproduction.as_deref()
        };
        let new_root = if updates.root_cause.is_some() {
            updates.root_cause.as_deref()
        } else {
            task.root_cause.as_deref()
        };
        let new_refs = updates.context_refs.as_deref().unwrap_or(&task.context_refs);
        let new_files = updates.files.as_deref().unwrap_or(&task.files);
        let new_tags = updates.tags.as_deref().unwrap_or(&task.tags);
        let history_json = serde_json::to_string(&history)?;

        conn.execute(
            "UPDATE tasks SET \
                title=?1, description=?2, category=?3, status=?4, slice_type=?5, \
                acceptance_criteria=?6, decisions=?7, implementation_plan=?8, \
                execution_record=?9, reproduction=?10, root_cause=?11, \
                context_refs=?12, files=?13, tags=?14, history=?15, updated_at=?16 \
            WHERE id=?17",
            rusqlite::params![
                new_title,
                new_desc,
                new_cat,
                new_status,
                new_slice,
                new_ac,
                new_dec,
                new_impl,
                new_exec,
                new_repro,
                new_root,
                new_refs,
                new_files,
                new_tags,
                history_json,
                now,
                task.id.as_bytes().as_slice(),
            ],
        )?;

        get_task_by_uuid(&conn, &task.id)?
            .ok_or_else(|| YojanaError::NotFound("updated task".into()))
    }

    // --- Edge methods ---

    pub fn create_edge(
        &self,
        source_task_id: Uuid,
        target_task_id: Uuid,
        edge_type: &str,
        note: Option<&str>,
    ) -> Result<EdgeRow, YojanaError> {
        if !VALID_EDGE_TYPES.contains(&edge_type) {
            return Err(YojanaError::InvalidInput(format!(
                "invalid edge_type '{edge_type}'; valid: {}",
                VALID_EDGE_TYPES.join(", ")
            )));
        }

        let conn = self.conn.lock();

        get_task_by_uuid(&conn, &source_task_id)?
            .ok_or_else(|| YojanaError::NotFound(format!("source task '{source_task_id}'")))?;
        get_task_by_uuid(&conn, &target_task_id)?
            .ok_or_else(|| YojanaError::NotFound(format!("target task '{target_task_id}'")))?;

        if edge_type == "depends_on" {
            let existing = load_depends_on_edges(&conn)?;
            crate::graph::would_cycle(&existing, source_task_id, target_task_id)?;
        }

        let id = Uuid::now_v7();
        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "INSERT INTO task_edges (id, source_task_id, target_task_id, edge_type, note, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id.as_bytes().as_slice(),
                source_task_id.as_bytes().as_slice(),
                target_task_id.as_bytes().as_slice(),
                edge_type,
                note,
                now,
            ],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(ref err, _)
                if err.code == rusqlite::ffi::ErrorCode::ConstraintViolation =>
            {
                YojanaError::Conflict(format!(
                    "edge ({source_task_id}, {target_task_id}, {edge_type}) already exists"
                ))
            }
            other => YojanaError::Db(other),
        })?;

        get_edge_by_id(&conn, &id)?
            .ok_or_else(|| YojanaError::NotFound("just-created edge".into()))
    }

    pub fn delete_edge(&self, id: &Uuid) -> Result<(), YojanaError> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM task_edges WHERE id = ?1",
            rusqlite::params![id.as_bytes().as_slice()],
        )?;
        if deleted == 0 {
            return Err(YojanaError::NotFound(format!("edge '{id}'")));
        }
        Ok(())
    }

    pub fn list_edges_for_task(&self, task_id: &Uuid) -> Result<Vec<EdgeRow>, YojanaError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_task_id, target_task_id, edge_type, note, created_at \
             FROM task_edges WHERE source_task_id = ?1 OR target_task_id = ?1 \
             ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![task_id.as_bytes().as_slice()], map_edge_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

fn load_depends_on_edges(conn: &Connection) -> Result<Vec<(Uuid, Uuid)>, YojanaError> {
    let mut stmt = conn.prepare(
        "SELECT source_task_id, target_task_id FROM task_edges WHERE edge_type = 'depends_on'",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let src = uuid_from_blob(row, "source_task_id")?;
            let tgt = uuid_from_blob(row, "target_task_id")?;
            Ok((src, tgt))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn map_edge_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EdgeRow> {
    Ok(EdgeRow {
        id: uuid_from_blob(row, "id")?,
        source_task_id: uuid_from_blob(row, "source_task_id")?,
        target_task_id: uuid_from_blob(row, "target_task_id")?,
        edge_type: row.get("edge_type")?,
        note: row.get("note")?,
        created_at: row.get("created_at")?,
    })
}

fn get_edge_by_id(conn: &Connection, id: &Uuid) -> Result<Option<EdgeRow>, YojanaError> {
    let mut stmt = conn.prepare(
        "SELECT id, source_task_id, target_task_id, edge_type, note, created_at \
         FROM task_edges WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(rusqlite::params![id.as_bytes().as_slice()], map_edge_row)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Db {
        Db::open_in_memory().unwrap()
    }

    // --- Project tests ---

    #[test]
    fn create_and_get_project() {
        let db = test_db();
        let p = db.create_project("test-proj", "Test Project", "A test").unwrap();
        assert_eq!(p.slug, "test-proj");
        assert_eq!(p.title, "Test Project");
        assert_eq!(p.description, "A test");
        assert_eq!(p.status, "active");

        let by_id = db.get_project(Some(&p.id.to_string()), None).unwrap().unwrap();
        assert_eq!(by_id.slug, "test-proj");

        let by_slug = db.get_project(None, Some("test-proj")).unwrap().unwrap();
        assert_eq!(by_slug.id, p.id);
    }

    #[test]
    fn slug_uniqueness() {
        let db = test_db();
        db.create_project("dupe", "First", "").unwrap();
        let err = db.create_project("dupe", "Second", "").unwrap_err();
        assert!(matches!(err, YojanaError::Conflict(_)));
    }

    #[test]
    fn list_with_status_filter() {
        let db = test_db();
        db.create_project("a", "A", "").unwrap();
        db.create_project("b", "B", "").unwrap();

        let all = db.list_projects(None).unwrap();
        assert_eq!(all.len(), 2);

        let active = db.list_projects(Some("active")).unwrap();
        assert_eq!(active.len(), 2);

        let paused = db.list_projects(Some("paused")).unwrap();
        assert_eq!(paused.len(), 0);
    }

    #[test]
    fn update_records_history() {
        let db = test_db();
        let p = db.create_project("test", "Original", "desc").unwrap();

        let updated = db
            .update_project(
                Some(&p.id.to_string()),
                None,
                ProjectUpdates {
                    title: Some("New Title".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "New Title");
        assert_eq!(updated.description, "desc");

        let history: Vec<HistoryEntry> = serde_json::from_str(&updated.history).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].kind, "updated");
    }

    #[test]
    fn update_status_records_history() {
        let db = test_db();
        db.create_project("test", "Test", "").unwrap();

        let updated = db
            .update_project(
                None,
                Some("test"),
                ProjectUpdates {
                    status: Some("paused".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.status, "paused");

        let history: Vec<HistoryEntry> = serde_json::from_str(&updated.history).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].kind, "status_changed");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let db = test_db();
        let result = db.get_project(None, Some("nope")).unwrap();
        assert!(result.is_none());
    }

    // --- Task tests ---

    fn create_test_task(db: &Db, project_slug: &str, title: &str) -> TaskRow {
        let p = db
            .get_project(None, Some(project_slug))
            .unwrap()
            .unwrap();
        db.create_task(CreateTaskParams {
            project_id: p.id,
            project_slug: p.slug,
            title: title.to_string(),
            description: String::new(),
            category: None,
            slice_type: None,
            acceptance_criteria: "[]".into(),
            decisions: "[]".into(),
            context_refs: "[]".into(),
            files: "[]".into(),
            tags: "[]".into(),
            implementation_plan: None,
            execution_record: None,
            reproduction: None,
            root_cause: None,
        })
        .unwrap()
    }

    #[test]
    fn create_task_with_sequence_numbers() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();

        let t1 = create_test_task(&db, "proj", "First");
        let t2 = create_test_task(&db, "proj", "Second");
        let t3 = create_test_task(&db, "proj", "Third");

        assert_eq!(t1.sequence_number, 1);
        assert_eq!(t2.sequence_number, 2);
        assert_eq!(t3.sequence_number, 3);
        assert_eq!(t1.status, "needs-triage");
        assert_eq!(t1.project_slug, "proj");
    }

    #[test]
    fn sequence_numbers_are_per_project() {
        let db = test_db();
        db.create_project("alpha", "Alpha", "").unwrap();
        db.create_project("beta", "Beta", "").unwrap();

        let a1 = create_test_task(&db, "alpha", "A1");
        let b1 = create_test_task(&db, "beta", "B1");
        let a2 = create_test_task(&db, "alpha", "A2");

        assert_eq!(a1.sequence_number, 1);
        assert_eq!(b1.sequence_number, 1);
        assert_eq!(a2.sequence_number, 2);
    }

    #[test]
    fn get_task_by_uuid() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t = create_test_task(&db, "proj", "Task");

        let fetched = db.get_task(&t.id.to_string()).unwrap().unwrap();
        assert_eq!(fetched.title, "Task");
        assert_eq!(fetched.project_slug, "proj");
    }

    #[test]
    fn get_task_by_slug_seq() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        create_test_task(&db, "proj", "First");
        let t2 = create_test_task(&db, "proj", "Second");

        let fetched = db.get_task("proj/2").unwrap().unwrap();
        assert_eq!(fetched.id, t2.id);
        assert_eq!(fetched.title, "Second");
    }

    #[test]
    fn update_task_partial() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t = create_test_task(&db, "proj", "Original");

        let updated = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    title: Some("Changed".into()),
                    category: Some("bug".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Changed");
        assert_eq!(updated.category.as_deref(), Some("bug"));
        assert_eq!(updated.status, "needs-triage"); // unchanged
    }

    #[test]
    fn json_round_trip() {
        let db = test_db();
        let p = db.create_project("proj", "Project", "").unwrap();

        let ac = serde_json::to_string(&vec![
            serde_json::json!({"id": "1", "text": "it works", "done": false}),
        ])
        .unwrap();
        let refs = serde_json::to_string(&vec![
            serde_json::json!({"type": "git:commit", "value": "abc123"}),
        ])
        .unwrap();
        let tags = serde_json::to_string(&vec!["infra", "urgent"]).unwrap();

        let t = db
            .create_task(CreateTaskParams {
                project_id: p.id,
                project_slug: p.slug,
                title: "JSON test".into(),
                description: String::new(),
                category: Some("enhancement".into()),
                slice_type: Some("AFK".into()),
                acceptance_criteria: ac,
                decisions: "[]".into(),
                context_refs: refs,
                files: "[]".into(),
                tags,
                implementation_plan: Some("do the thing".into()),
                execution_record: None,
                reproduction: None,
                root_cause: None,
            })
            .unwrap();

        let fetched = db.get_task(&t.id.to_string()).unwrap().unwrap();
        let ac_parsed: Vec<serde_json::Value> =
            serde_json::from_str(&fetched.acceptance_criteria).unwrap();
        assert_eq!(ac_parsed.len(), 1);
        assert_eq!(ac_parsed[0]["text"], "it works");

        let refs_parsed: Vec<serde_json::Value> =
            serde_json::from_str(&fetched.context_refs).unwrap();
        assert_eq!(refs_parsed[0]["type"], "git:commit");

        let tags_parsed: Vec<String> = serde_json::from_str(&fetched.tags).unwrap();
        assert_eq!(tags_parsed, vec!["infra", "urgent"]);

        assert_eq!(fetched.implementation_plan.as_deref(), Some("do the thing"));
        assert_eq!(fetched.category.as_deref(), Some("enhancement"));
        assert_eq!(fetched.slice_type.as_deref(), Some("AFK"));
    }

    #[test]
    fn cascade_delete_project_removes_tasks() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t = create_test_task(&db, "proj", "Task");
        let task_id = t.id.to_string();

        // Delete project by dropping and recreating the table... actually
        // we don't have a delete_project method. Use raw SQL.
        {
            let conn = db.conn.lock();
            let p = get_by_slug(&conn, "proj").unwrap().unwrap();
            conn.execute(
                "DELETE FROM projects WHERE id = ?1",
                rusqlite::params![p.id.as_bytes().as_slice()],
            )
            .unwrap();
        }

        let result = db.get_task(&task_id).unwrap();
        assert!(result.is_none());
    }

    // --- Slice 03: state machine integration ---

    #[test]
    fn valid_status_transition_succeeds() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t = create_test_task(&db, "proj", "Task");
        assert_eq!(t.status, "needs-triage");

        let updated = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    status: Some("ready-for-agent".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(updated.status, "ready-for-agent");

        let history: Vec<HistoryEntry> = serde_json::from_str(&updated.history).unwrap();
        let status_entries: Vec<_> = history
            .iter()
            .filter(|h| h.kind == "status_changed")
            .collect();
        assert_eq!(status_entries.len(), 1);
        assert_eq!(status_entries[0].payload["from"], "needs-triage");
        assert_eq!(status_entries[0].payload["to"], "ready-for-agent");
    }

    #[test]
    fn invalid_status_transition_rejected() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t = create_test_task(&db, "proj", "Task");

        let err = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    status: Some("done".into()),
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid transition"));
    }

    #[test]
    fn noop_status_update_skips_validation() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t = create_test_task(&db, "proj", "Task");

        let updated = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    status: Some("needs-triage".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let history: Vec<HistoryEntry> = serde_json::from_str(&updated.history).unwrap();
        let status_entries: Vec<_> = history
            .iter()
            .filter(|h| h.kind == "status_changed")
            .collect();
        assert_eq!(status_entries.len(), 0);
    }

    #[test]
    fn non_status_update_bypasses_state_machine() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t = create_test_task(&db, "proj", "Task");

        let updated = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    title: Some("New title".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(updated.title, "New title");
        assert_eq!(updated.status, "needs-triage");
    }

    #[test]
    fn done_is_terminal_except_reopen() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t = create_test_task(&db, "proj", "Task");
        let id = t.id.to_string();

        db.update_task(&id, TaskUpdates { status: Some("ready-for-agent".into()), ..Default::default() }).unwrap();
        db.update_task(&id, TaskUpdates { status: Some("in_progress".into()), ..Default::default() }).unwrap();
        db.update_task(&id, TaskUpdates { status: Some("done".into()), ..Default::default() }).unwrap();

        let err = db.update_task(&id, TaskUpdates { status: Some("in_progress".into()), ..Default::default() }).unwrap_err();
        assert!(err.to_string().contains("invalid transition"));

        let reopened = db.update_task(&id, TaskUpdates { status: Some("needs-triage".into()), ..Default::default() }).unwrap();
        assert_eq!(reopened.status, "needs-triage");
    }

    // --- Slice 04: edge CRUD ---

    #[test]
    fn create_and_list_edges() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        let edge = db
            .create_edge(t1.id, t2.id, "depends_on", Some("t1 needs t2"))
            .unwrap();
        assert_eq!(edge.source_task_id, t1.id);
        assert_eq!(edge.target_task_id, t2.id);
        assert_eq!(edge.edge_type, "depends_on");
        assert_eq!(edge.note.as_deref(), Some("t1 needs t2"));

        let edges = db.list_edges_for_task(&t1.id).unwrap();
        assert_eq!(edges.len(), 1);

        let edges = db.list_edges_for_task(&t2.id).unwrap();
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn delete_edge() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        let edge = db.create_edge(t1.id, t2.id, "relates_to", None).unwrap();
        db.delete_edge(&edge.id).unwrap();

        let edges = db.list_edges_for_task(&t1.id).unwrap();
        assert!(edges.is_empty());
    }

    #[test]
    fn delete_nonexistent_edge_errors() {
        let db = test_db();
        let fake_id = Uuid::now_v7();
        let err = db.delete_edge(&fake_id).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn duplicate_edge_rejected() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();
        let err = db.create_edge(t1.id, t2.id, "depends_on", None).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn same_pair_different_types_allowed() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();
        db.create_edge(t1.id, t2.id, "relates_to", None).unwrap();

        let edges = db.list_edges_for_task(&t1.id).unwrap();
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn cycle_detection_rejects_direct_cycle() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();
        let err = db.create_edge(t2.id, t1.id, "depends_on", None).unwrap_err();
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn cycle_detection_rejects_multi_hop() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");
        let t3 = create_test_task(&db, "proj", "Task 3");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();
        db.create_edge(t2.id, t3.id, "depends_on", None).unwrap();
        let err = db.create_edge(t3.id, t1.id, "depends_on", None).unwrap_err();
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn non_dependency_edges_skip_cycle_check() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        db.create_edge(t1.id, t2.id, "relates_to", None).unwrap();
        db.create_edge(t2.id, t1.id, "relates_to", None).unwrap();
    }

    #[test]
    fn invalid_edge_type_rejected() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        let err = db.create_edge(t1.id, t2.id, "blocks", None).unwrap_err();
        assert!(err.to_string().contains("invalid edge_type"));
    }

    #[test]
    fn cross_project_edges() {
        let db = test_db();
        db.create_project("alpha", "Alpha", "").unwrap();
        db.create_project("beta", "Beta", "").unwrap();
        let t1 = create_test_task(&db, "alpha", "Task A");
        let t2 = create_test_task(&db, "beta", "Task B");

        let edge = db
            .create_edge(t1.id, t2.id, "motivated_by", None)
            .unwrap();
        assert_eq!(edge.source_task_id, t1.id);
        assert_eq!(edge.target_task_id, t2.id);
    }

    #[test]
    fn cascade_delete_task_removes_edges() {
        let db = test_db();
        db.create_project("proj", "Project", "").unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");
        let t3 = create_test_task(&db, "proj", "Task 3");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();
        db.create_edge(t3.id, t2.id, "relates_to", None).unwrap();

        {
            let conn = db.conn.lock();
            conn.execute(
                "DELETE FROM tasks WHERE id = ?1",
                rusqlite::params![t2.id.as_bytes().as_slice()],
            )
            .unwrap();
        }

        let edges = db.list_edges_for_task(&t1.id).unwrap();
        assert!(edges.is_empty());
        let edges = db.list_edges_for_task(&t3.id).unwrap();
        assert!(edges.is_empty());
    }
}
