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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

// --- Project types ---

#[derive(Debug)]
pub struct ProjectRow {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub parent_id: Option<Uuid>,
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

// --- Arc types ---

pub const VALID_ARC_STATUSES: &[&str] = &["active", "paused", "completed", "abandoned"];
pub const VALID_PHASE_STATUSES: &[&str] = &["pending", "active", "completed", "skipped"];
pub const VALID_PHASE_GATES: &[&str] = &["auto", "manual"];

#[derive(Debug)]
pub struct ArcRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_slug: String,
    pub sequence_number: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub phases: String,
    pub tags: String,
    pub context_refs: String,
    pub history: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct CreateArcParams {
    pub project_id: Uuid,
    pub project_slug: String,
    pub title: String,
    pub description: String,
    pub phases: String,
    pub tags: String,
    pub context_refs: String,
}

#[derive(Debug, Default)]
pub struct ArcUpdates {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub tags: Option<String>,
    pub context_refs: Option<String>,
}

enum ArcIdentifier {
    Uuid(Uuid),
    SlugSeq(String, i64),
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
    pub completed_at: Option<i64>,
    pub arc_id: Option<Uuid>,
    pub arc_phase: Option<String>,
}

pub struct CreateTaskParams {
    pub project_id: Uuid,
    pub project_slug: String,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub status: Option<String>,
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
    pub arc_id: Option<Uuid>,
    pub arc_phase: Option<String>,
}

#[derive(Debug, Default)]
pub struct TaskUpdates {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<Option<String>>,
    pub status: Option<String>,
    /// Skip state-machine validation for status changes (CLI override).
    pub force_status: bool,
    pub slice_type: Option<Option<String>>,
    pub acceptance_criteria: Option<String>,
    pub decisions: Option<String>,
    pub implementation_plan: Option<Option<String>>,
    pub execution_record: Option<Option<String>>,
    pub reproduction: Option<Option<String>>,
    pub root_cause: Option<Option<String>>,
    pub context_refs: Option<String>,
    pub files: Option<String>,
    pub tags: Option<String>,
    pub arc_id: Option<Option<Uuid>>,
    pub arc_phase: Option<Option<String>>,
}

enum TaskIdentifier {
    Uuid(Uuid),
    SlugSeq(String, i64),
}

pub const DEFAULT_PAGE_LIMIT: i64 = 100;

#[derive(Debug, Default)]
pub struct TaskQueryFilter {
    pub project_ids: Option<Vec<Uuid>>,
    pub status: Option<String>,
    pub category: Option<String>,
    pub slice_type: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// When set, terminal-status tasks ('done', 'wontfix') are only included
    /// if their completed_at >= this timestamp (millis). Non-terminal tasks
    /// are unaffected.
    pub include_terminal_after: Option<i64>,
    pub arc_id: Option<Uuid>,
}

/// Statuses that count as "terminal" for default-hide filtering.
pub const TERMINAL_STATUSES: &[&str] = &["done", "wontfix"];

// --- Project statuses ---

const VALID_PROJECT_STATUSES: &[&str] = &["active", "production", "paused", "archived"];

// --- Edge types ---

pub const VALID_EDGE_TYPES: &[&str] = &[
    "depends_on",
    "relates_to",
    "supersedes",
    "refines",
    "motivated_by",
];

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
    let parent_id = row
        .get::<_, Option<Vec<u8>>>("parent_id")?
        .map(|bytes| {
            Uuid::from_slice(&bytes).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Blob,
                    Box::new(e),
                )
            })
        })
        .transpose()?;
    Ok(ProjectRow {
        id,
        slug: row.get("slug")?,
        title: row.get("title")?,
        description: row.get("description")?,
        status: row.get("status")?,
        parent_id,
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
    t.context_refs, t.files, t.tags, t.history, t.created_at, t.updated_at, \
    t.completed_at, t.arc_id, t.arc_phase \
    FROM tasks t JOIN projects p ON t.project_id = p.id";

fn uuid_from_blob(row: &rusqlite::Row<'_>, col: &str) -> rusqlite::Result<Uuid> {
    let bytes: Vec<u8> = row.get(col)?;
    Uuid::from_slice(&bytes).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e))
    })
}

fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRow> {
    let arc_id = row
        .get::<_, Option<Vec<u8>>>("arc_id")?
        .map(|bytes| {
            Uuid::from_slice(&bytes).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Blob,
                    Box::new(e),
                )
            })
        })
        .transpose()?;
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
        completed_at: row.get("completed_at")?,
        arc_id,
        arc_phase: row.get("arc_phase")?,
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
    if let Some((slug, num_str)) = s.rsplit_once('/')
        && let Ok(num) = num_str.parse::<i64>()
    {
        return Ok(TaskIdentifier::SlugSeq(slug.to_string(), num));
    }
    Err(YojanaError::InvalidInput(format!(
        "invalid task identifier '{s}'; expected UUID or 'project-slug/N'"
    )))
}

fn next_sequence_number(conn: &Connection, project_id: &Uuid) -> Result<i64, YojanaError> {
    let mut stmt = conn
        .prepare("SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM tasks WHERE project_id = ?1")?;
    let seq: i64 = stmt.query_row(rusqlite::params![project_id.as_bytes().as_slice()], |row| {
        row.get(0)
    })?;
    Ok(seq)
}

fn resolve_task(conn: &Connection, identifier: &str) -> Result<TaskRow, YojanaError> {
    let row = match parse_task_identifier(identifier)? {
        TaskIdentifier::Uuid(id) => get_task_by_uuid(conn, &id)?,
        TaskIdentifier::SlugSeq(slug, seq) => get_task_by_slug_seq(conn, &slug, seq)?,
    };
    row.ok_or_else(|| YojanaError::NotFound(format!("task '{identifier}'")))
}

// --- Arc helpers ---

const ARC_SELECT: &str = "\
    SELECT a.id, a.project_id, p.slug AS project_slug, a.sequence_number, \
    a.title, a.description, a.status, a.phases, a.tags, a.context_refs, \
    a.history, a.created_at, a.updated_at \
    FROM arcs a JOIN projects p ON a.project_id = p.id";

fn map_arc_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArcRow> {
    Ok(ArcRow {
        id: uuid_from_blob(row, "id")?,
        project_id: uuid_from_blob(row, "project_id")?,
        project_slug: row.get("project_slug")?,
        sequence_number: row.get("sequence_number")?,
        title: row.get("title")?,
        description: row.get("description")?,
        status: row.get("status")?,
        phases: row.get("phases")?,
        tags: row.get("tags")?,
        context_refs: row.get("context_refs")?,
        history: row.get("history")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn get_arc_by_uuid(conn: &Connection, id: &Uuid) -> Result<Option<ArcRow>, YojanaError> {
    let sql = format!("{ARC_SELECT} WHERE a.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let row = stmt
        .query_row(rusqlite::params![id.as_bytes().as_slice()], map_arc_row)
        .optional()?;
    Ok(row)
}

fn get_arc_by_slug_seq(
    conn: &Connection,
    slug: &str,
    seq: i64,
) -> Result<Option<ArcRow>, YojanaError> {
    let sql = format!("{ARC_SELECT} WHERE p.slug = ?1 AND a.sequence_number = ?2");
    let mut stmt = conn.prepare(&sql)?;
    let row = stmt
        .query_row(rusqlite::params![slug, seq], map_arc_row)
        .optional()?;
    Ok(row)
}

fn parse_arc_identifier(s: &str) -> Result<ArcIdentifier, YojanaError> {
    if let Ok(uuid) = Uuid::parse_str(s) {
        return Ok(ArcIdentifier::Uuid(uuid));
    }
    if let Some((slug, num_str)) = s.rsplit_once('/')
        && let Some(num_str) = num_str.strip_prefix('~')
        && let Ok(num) = num_str.parse::<i64>()
    {
        return Ok(ArcIdentifier::SlugSeq(slug.to_string(), num));
    }
    Err(YojanaError::InvalidInput(format!(
        "invalid arc identifier '{s}'; expected UUID or 'project-slug/~N'"
    )))
}

fn next_arc_sequence_number(conn: &Connection, project_id: &Uuid) -> Result<i64, YojanaError> {
    let mut stmt = conn
        .prepare("SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM arcs WHERE project_id = ?1")?;
    let seq: i64 = stmt.query_row(rusqlite::params![project_id.as_bytes().as_slice()], |row| {
        row.get(0)
    })?;
    Ok(seq)
}

fn resolve_arc(conn: &Connection, identifier: &str) -> Result<ArcRow, YojanaError> {
    let row = match parse_arc_identifier(identifier)? {
        ArcIdentifier::Uuid(id) => get_arc_by_uuid(conn, &id)?,
        ArcIdentifier::SlugSeq(slug, seq) => get_arc_by_slug_seq(conn, &slug, seq)?,
    };
    row.ok_or_else(|| YojanaError::NotFound(format!("arc '{identifier}'")))
}

fn validate_phases(phases: &[serde_json::Value]) -> Result<(), YojanaError> {
    if phases.is_empty() {
        return Err(YojanaError::InvalidInput(
            "at least one phase required".into(),
        ));
    }
    let mut names = std::collections::HashSet::new();
    for phase in phases {
        let name = phase
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| YojanaError::InvalidInput("phase must have a 'name' string".into()))?;
        if !names.insert(name.to_string()) {
            return Err(YojanaError::InvalidInput(format!(
                "duplicate phase name '{name}'"
            )));
        }
        if let Some(st) = phase.get("slice_type").and_then(|s| s.as_str())
            && !["AFK", "HITL"].contains(&st)
        {
            return Err(YojanaError::InvalidInput(format!(
                "invalid phase slice_type '{st}'; valid: AFK, HITL"
            )));
        }
        if let Some(gate) = phase.get("gate").and_then(|g| g.as_str())
            && !VALID_PHASE_GATES.contains(&gate)
        {
            return Err(YojanaError::InvalidInput(format!(
                "invalid phase gate '{gate}'; valid: {}",
                VALID_PHASE_GATES.join(", ")
            )));
        }
        if let Some(status) = phase.get("status").and_then(|s| s.as_str())
            && !VALID_PHASE_STATUSES.contains(&status)
        {
            return Err(YojanaError::InvalidInput(format!(
                "invalid phase status '{status}'; valid: {}",
                VALID_PHASE_STATUSES.join(", ")
            )));
        }
    }
    Ok(())
}

fn apply_phase_defaults(phases: &[serde_json::Value]) -> Vec<serde_json::Value> {
    phases
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut phase = p.clone();
            let obj = phase.as_object_mut().expect(
                "invariant: validate_phases runs first and requires a 'name' string on every \
                 phase, which only a JSON object can carry",
            );
            if !obj.contains_key("status") {
                let status = if i == 0 { "active" } else { "pending" };
                obj.insert("status".into(), serde_json::Value::String(status.into()));
            }
            if !obj.contains_key("gate") {
                obj.insert("gate".into(), serde_json::Value::String("manual".into()));
            }
            phase
        })
        .collect()
}

/// Set a key on a JSON object in place.
///
/// Every caller passes a value built by `json!({...})` or a phase that
/// `apply_phase_defaults` already proved to be an object.
fn set_field(value: &mut serde_json::Value, key: &str, field: serde_json::Value) {
    value
        .as_object_mut()
        .expect("invariant: caller passes a JSON object")
        .insert(key.to_string(), field);
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
        const MIGRATIONS: &[(&str, &str)] = &[
            (
                "0001_initial",
                include_str!("../migrations/0001_initial.sql"),
            ),
            ("0002_tasks", include_str!("../migrations/0002_tasks.sql")),
            ("0003_edges", include_str!("../migrations/0003_edges.sql")),
            (
                "0004_conversations",
                include_str!("../migrations/0004_conversations.sql"),
            ),
            (
                "0005_in_progress_rename",
                include_str!("../migrations/0005_in-progress-rename.sql"),
            ),
            (
                "0006_parent_projects",
                include_str!("../migrations/0006_parent-projects.sql"),
            ),
            (
                "0007_completed_at",
                include_str!("../migrations/0007_completed_at.sql"),
            ),
            // 0008 added the project handoff column, 0011 drops it again (see
            // yojana/41). 0008 stays listed so databases that already recorded
            // it remain consistent; on a fresh database the pair cancels.
            (
                "0008_project_handoff",
                include_str!("../migrations/0008_project-handoff.sql"),
            ),
            ("0009_arcs", include_str!("../migrations/0009_arcs.sql")),
            (
                "0010_arc_task_index",
                include_str!("../migrations/0010_arc-task-index.sql"),
            ),
            (
                "0011_drop_project_handoff",
                include_str!("../migrations/0011_drop-project-handoff.sql"),
            ),
            (
                "0012_normalize_acceptance_criteria",
                include_str!("../migrations/0012_normalize-acceptance-criteria.sql"),
            ),
            (
                "0013_canonical_acceptance_criteria",
                include_str!("../migrations/0013_canonical-acceptance-criteria.sql"),
            ),
        ];

        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _yojana_migrations (
                name TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
        )?;

        for (name, sql) in MIGRATIONS {
            let already_applied: bool = conn
                .prepare("SELECT 1 FROM _yojana_migrations WHERE name = ?1")?
                .query_row(rusqlite::params![name], |_| Ok(true))
                .optional()?
                .unwrap_or(false);

            if !already_applied {
                conn.execute_batch(sql)?;
                conn.execute(
                    "INSERT INTO _yojana_migrations (name, applied_at) VALUES (?1, ?2)",
                    rusqlite::params![name, chrono::Utc::now().timestamp_millis()],
                )?;
                tracing::info!("applied migration: {name}");
            }
        }
        Ok(())
    }

    // --- Project methods ---

    pub fn create_project(
        &self,
        slug: &str,
        title: &str,
        description: &str,
        parent_id: Option<Uuid>,
        actor: &str,
    ) -> Result<ProjectRow, YojanaError> {
        let conn = self.conn.lock();
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().timestamp_millis();
        let history = serde_json::to_string(&vec![HistoryEntry {
            ts: now,
            kind: "project_created".into(),
            payload: serde_json::json!({}),
            actor: Some(actor.to_string()),
        }])?;

        let parent_bytes = parent_id.map(|pid| pid.as_bytes().to_vec());
        conn.execute(
            "INSERT INTO projects (id, slug, title, description, status, parent_id, history, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?7, ?7)",
            rusqlite::params![id.as_bytes().as_slice(), slug, title, description, parent_bytes, history, now],
        )
        .map_err(|e| {
            if let rusqlite::Error::SqliteFailure(ref err, _) = e
                && err.extended_code == 2067 {
                    return YojanaError::Conflict(format!(
                        "project slug '{slug}' already exists"
                    ));
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

    /// List projects with optional filters.
    /// `parent_filter`: None = no parent filter, Some(None) = roots only, Some(Some(id)) = children of id.
    pub fn list_projects(
        &self,
        status: Option<&str>,
        parent_filter: Option<Option<&Uuid>>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<ProjectRow>, YojanaError> {
        let conn = self.conn.lock();
        let lim = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
        let off = offset.unwrap_or(0);

        let mut sql = String::from("SELECT * FROM projects");
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(status) = status {
            params.push(Box::new(status.to_string()));
            conditions.push(format!("status = ?{}", params.len()));
        }

        match parent_filter {
            Some(None) => {
                conditions.push("parent_id IS NULL".to_string());
            }
            Some(Some(pid)) => {
                params.push(Box::new(pid.as_bytes().to_vec()));
                conditions.push(format!("parent_id = ?{}", params.len()));
            }
            None => {}
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY created_at");

        params.push(Box::new(lim));
        sql.push_str(&format!(" LIMIT ?{}", params.len()));
        params.push(Box::new(off));
        sql.push_str(&format!(" OFFSET ?{}", params.len()));

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), map_project_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_projects_slim(
        &self,
        status: Option<&str>,
        parent_filter: Option<Option<&Uuid>>,
    ) -> Result<Vec<crate::tools::project::ProjectListItem>, YojanaError> {
        let conn = self.conn.lock();

        let mut sql = String::from(
            "SELECT p.id, p.slug, p.title, p.status, p.parent_id,              (SELECT COUNT(*) FROM projects c WHERE c.parent_id = p.id) AS child_count,              p.created_at, p.updated_at              FROM projects p",
        );
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(status) = status {
            params.push(Box::new(status.to_string()));
            conditions.push(format!("p.status = ?{}", params.len()));
        }

        match parent_filter {
            Some(None) => {
                conditions.push("p.parent_id IS NULL".to_string());
            }
            Some(Some(pid)) => {
                params.push(Box::new(pid.as_bytes().to_vec()));
                conditions.push(format!("p.parent_id = ?{}", params.len()));
            }
            None => {}
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY p.slug");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                let id = Uuid::from_slice(&id_bytes).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(e),
                    )
                })?;
                let parent_bytes: Option<Vec<u8>> = row.get(4)?;
                let parent_id = parent_bytes
                    .map(|bytes| {
                        Uuid::from_slice(&bytes).map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Blob,
                                Box::new(e),
                            )
                        })
                    })
                    .transpose()?;
                Ok(crate::tools::project::ProjectListItem {
                    id: id.to_string(),
                    slug: row.get(1)?,
                    title: row.get(2)?,
                    status: row.get(3)?,
                    parent_id: parent_id.map(|id| id.to_string()),
                    child_count: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_descendant_project_ids(&self, root_id: &Uuid) -> Result<Vec<Uuid>, YojanaError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "WITH RECURSIVE descendants(id) AS (
                SELECT id FROM projects WHERE parent_id = ?1
                UNION ALL
                SELECT p.id FROM projects p JOIN descendants d ON p.parent_id = d.id
            )
            SELECT id FROM descendants",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![root_id.as_bytes().as_slice()], |row| {
                let bytes: Vec<u8> = row.get(0)?;
                Ok(bytes)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut ids = Vec::with_capacity(rows.len());
        for bytes in rows {
            let id = Uuid::from_slice(&bytes)
                .map_err(|e| YojanaError::Internal(format!("invalid uuid in descendants: {e}")))?;
            ids.push(id);
        }
        Ok(ids)
    }

    pub fn project_ids_with_descendants(
        &self,
        project_id: &Uuid,
    ) -> Result<Vec<Uuid>, YojanaError> {
        let mut ids = vec![*project_id];
        ids.extend(self.list_descendant_project_ids(project_id)?);
        Ok(ids)
    }

    pub fn update_project(
        &self,
        id: Option<&str>,
        slug: Option<&str>,
        updates: ProjectUpdates,
        actor: &str,
    ) -> Result<ProjectRow, YojanaError> {
        let conn = self.conn.lock();
        let project = resolve_project(&conn, id, slug)?;
        let now = chrono::Utc::now().timestamp_millis();
        let mut history: Vec<HistoryEntry> = serde_json::from_str(&project.history)?;

        if let Some(ref new_title) = updates.title
            && new_title != &project.title
        {
            history.push(HistoryEntry {
                    ts: now,
                    kind: "updated".into(),
                    payload: serde_json::json!({"field": "title", "from": project.title, "to": new_title}),
                    actor: Some(actor.to_string()),
                });
        }
        if let Some(ref new_desc) = updates.description
            && new_desc != &project.description
        {
            history.push(HistoryEntry {
                ts: now,
                kind: "updated".into(),
                payload: serde_json::json!({"field": "description"}),
                actor: Some(actor.to_string()),
            });
        }
        if let Some(ref new_status) = updates.status {
            if !VALID_PROJECT_STATUSES.contains(&new_status.as_str()) {
                return Err(YojanaError::InvalidInput(format!(
                    "invalid project status '{new_status}'; valid: {}",
                    VALID_PROJECT_STATUSES.join(", ")
                )));
            }
            if new_status != &project.status {
                history.push(HistoryEntry {
                    ts: now,
                    kind: "status_changed".into(),
                    payload: serde_json::json!({"from": project.status, "to": new_status}),
                    actor: Some(actor.to_string()),
                });
            }
        }

        let new_title = updates.title.as_deref().unwrap_or(&project.title);
        let new_desc = updates
            .description
            .as_deref()
            .unwrap_or(&project.description);
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

    pub fn create_task(
        &self,
        params: CreateTaskParams,
        actor: &str,
    ) -> Result<TaskRow, YojanaError> {
        let conn = self.conn.lock();
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().timestamp_millis();
        let seq = next_sequence_number(&conn, &params.project_id)?;
        let history = serde_json::to_string(&vec![HistoryEntry {
            ts: now,
            kind: "task_created".into(),
            payload: serde_json::json!({"sequence_number": seq, "project": params.project_slug}),
            actor: Some(actor.to_string()),
        }])?;

        let status = params.status.as_deref().unwrap_or("needs-triage");
        if !state::valid_status(status) {
            return Err(YojanaError::InvalidInput(format!(
                "unknown status '{status}'"
            )));
        }

        conn.execute(
            "INSERT INTO tasks (\
                id, project_id, sequence_number, title, description, \
                category, status, slice_type, acceptance_criteria, decisions, \
                implementation_plan, execution_record, reproduction, root_cause, \
                context_refs, files, tags, history, created_at, updated_at, \
                arc_id, arc_phase\
            ) VALUES (\
                ?1, ?2, ?3, ?4, ?5, \
                ?6, ?7, ?8, ?9, ?10, \
                ?11, ?12, ?13, ?14, \
                ?15, ?16, ?17, ?18, ?19, ?19, \
                ?20, ?21\
            )",
            rusqlite::params![
                id.as_bytes().as_slice(),
                params.project_id.as_bytes().as_slice(),
                seq,
                params.title,
                params.description,
                params.category,
                status,
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
                params.arc_id.map(|id| id.as_bytes().to_vec()),
                params.arc_phase,
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
        actor: &str,
    ) -> Result<TaskRow, YojanaError> {
        let conn = self.conn.lock();
        let task = resolve_task(&conn, identifier)?;
        let now = chrono::Utc::now().timestamp_millis();
        let mut history: Vec<HistoryEntry> = serde_json::from_str(&task.history)?;

        let mut new_completed_at = task.completed_at;
        if let Some(ref new_status) = updates.status
            && new_status != &task.status
        {
            if updates.force_status {
                if !state::valid_status(new_status) {
                    return Err(YojanaError::InvalidInput(format!(
                        "unknown status '{new_status}'"
                    )));
                }
            } else {
                state::validate_transition(&task.status, new_status)?;
            }
            history.push(HistoryEntry {
                ts: now,
                kind: "status_changed".into(),
                payload: serde_json::json!({"from": task.status, "to": new_status}),
                actor: Some(actor.to_string()),
            });
            if TERMINAL_STATUSES.contains(&new_status.as_str()) {
                new_completed_at = Some(now);
            } else if TERMINAL_STATUSES.contains(&task.status.as_str()) {
                new_completed_at = None;
            }
        }
        if let Some(ref new_title) = updates.title
            && new_title != &task.title
        {
            history.push(HistoryEntry {
                ts: now,
                kind: "updated".into(),
                payload: serde_json::json!({"field": "title", "from": task.title, "to": new_title}),
                actor: Some(actor.to_string()),
            });
        }

        let new_title = updates.title.as_deref().unwrap_or(&task.title);
        let new_desc = updates.description.as_deref().unwrap_or(&task.description);
        let new_cat: Option<&str> = match &updates.category {
            None => task.category.as_deref(),
            Some(inner) => inner.as_deref(),
        };
        let new_status = updates.status.as_deref().unwrap_or(&task.status);
        let new_slice: Option<&str> = match &updates.slice_type {
            None => task.slice_type.as_deref(),
            Some(inner) => inner.as_deref(),
        };
        let new_ac = updates
            .acceptance_criteria
            .as_deref()
            .unwrap_or(&task.acceptance_criteria);
        let new_dec = updates.decisions.as_deref().unwrap_or(&task.decisions);
        let new_impl: Option<&str> = match &updates.implementation_plan {
            None => task.implementation_plan.as_deref(),
            Some(inner) => inner.as_deref(),
        };
        let new_exec: Option<&str> = match &updates.execution_record {
            None => task.execution_record.as_deref(),
            Some(inner) => inner.as_deref(),
        };
        let new_repro: Option<&str> = match &updates.reproduction {
            None => task.reproduction.as_deref(),
            Some(inner) => inner.as_deref(),
        };
        let new_root: Option<&str> = match &updates.root_cause {
            None => task.root_cause.as_deref(),
            Some(inner) => inner.as_deref(),
        };
        let new_refs = updates
            .context_refs
            .as_deref()
            .unwrap_or(&task.context_refs);
        let new_files = updates.files.as_deref().unwrap_or(&task.files);
        let new_tags = updates.tags.as_deref().unwrap_or(&task.tags);
        let new_arc_id: Option<Uuid> = match &updates.arc_id {
            None => task.arc_id,
            Some(inner) => *inner,
        };
        let new_arc_phase: Option<&str> = match &updates.arc_phase {
            None => task.arc_phase.as_deref(),
            Some(inner) => inner.as_deref(),
        };
        let history_json = serde_json::to_string(&history)?;

        conn.execute(
            "UPDATE tasks SET \
                title=?1, description=?2, category=?3, status=?4, slice_type=?5, \
                acceptance_criteria=?6, decisions=?7, implementation_plan=?8, \
                execution_record=?9, reproduction=?10, root_cause=?11, \
                context_refs=?12, files=?13, tags=?14, history=?15, updated_at=?16, \
                completed_at=?17, arc_id=?19, arc_phase=?20 \
            WHERE id=?18",
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
                new_completed_at,
                task.id.as_bytes().as_slice(),
                new_arc_id.map(|id| id.as_bytes().to_vec()),
                new_arc_phase,
            ],
        )?;

        get_task_by_uuid(&conn, &task.id)?
            .ok_or_else(|| YojanaError::NotFound("updated task".into()))
    }

    // --- Arc methods ---

    pub fn create_arc(&self, params: CreateArcParams, actor: &str) -> Result<ArcRow, YojanaError> {
        let input_phases: Vec<serde_json::Value> = serde_json::from_str(&params.phases)?;
        validate_phases(&input_phases)?;
        let phases_with_defaults = apply_phase_defaults(&input_phases);
        let phases_json = serde_json::to_string(&phases_with_defaults)?;

        let conn = self.conn.lock();
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().timestamp_millis();
        let seq = next_arc_sequence_number(&conn, &params.project_id)?;
        let history = serde_json::to_string(&vec![HistoryEntry {
            ts: now,
            kind: "arc_created".into(),
            payload: serde_json::json!({"sequence_number": seq, "project": params.project_slug}),
            actor: Some(actor.to_string()),
        }])?;

        conn.execute(
            "INSERT INTO arcs (\
                id, project_id, sequence_number, title, description, \
                status, phases, tags, context_refs, history, \
                created_at, updated_at\
            ) VALUES (\
                ?1, ?2, ?3, ?4, ?5, \
                ?6, ?7, ?8, ?9, ?10, \
                ?11, ?11\
            )",
            rusqlite::params![
                id.as_bytes().as_slice(),
                params.project_id.as_bytes().as_slice(),
                seq,
                params.title,
                params.description,
                "active",
                phases_json,
                params.tags,
                params.context_refs,
                history,
                now,
            ],
        )?;

        get_arc_by_uuid(&conn, &id)?.ok_or_else(|| YojanaError::NotFound("just-created arc".into()))
    }

    pub fn get_arc(&self, identifier: &str) -> Result<Option<ArcRow>, YojanaError> {
        let conn = self.conn.lock();
        match parse_arc_identifier(identifier)? {
            ArcIdentifier::Uuid(id) => get_arc_by_uuid(&conn, &id),
            ArcIdentifier::SlugSeq(slug, seq) => get_arc_by_slug_seq(&conn, &slug, seq),
        }
    }

    pub fn list_arcs_for_projects(&self, project_ids: &[Uuid]) -> Result<Vec<ArcRow>, YojanaError> {
        let conn = self.conn.lock();
        if project_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let placeholders: Vec<String> = project_ids
            .iter()
            .map(|pid| {
                params.push(Box::new(pid.as_bytes().to_vec()));
                format!("?{}", params.len())
            })
            .collect();
        let sql = format!(
            "{ARC_SELECT} WHERE a.project_id IN ({}) ORDER BY a.sequence_number ASC",
            placeholders.join(", ")
        );
        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), map_arc_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn update_arc(
        &self,
        identifier: &str,
        updates: ArcUpdates,
        actor: &str,
    ) -> Result<ArcRow, YojanaError> {
        let conn = self.conn.lock();
        let arc = resolve_arc(&conn, identifier)?;
        let now = chrono::Utc::now().timestamp_millis();
        let mut history: Vec<HistoryEntry> = serde_json::from_str(&arc.history)?;

        if let Some(ref new_title) = updates.title
            && new_title != &arc.title
        {
            history.push(HistoryEntry {
                ts: now,
                kind: "updated".into(),
                payload: serde_json::json!({"field": "title", "from": arc.title, "to": new_title}),
                actor: Some(actor.to_string()),
            });
        }
        if let Some(ref new_status) = updates.status
            && new_status != &arc.status
        {
            if !VALID_ARC_STATUSES.contains(&new_status.as_str()) {
                return Err(YojanaError::InvalidInput(format!(
                    "invalid arc status '{new_status}'; valid: {}",
                    VALID_ARC_STATUSES.join(", ")
                )));
            }
            history.push(HistoryEntry {
                ts: now,
                kind: "status_changed".into(),
                payload: serde_json::json!({"from": arc.status, "to": new_status}),
                actor: Some(actor.to_string()),
            });
        }
        if let Some(ref new_desc) = updates.description
            && new_desc != &arc.description
        {
            history.push(HistoryEntry {
                ts: now,
                kind: "updated".into(),
                payload: serde_json::json!({"field": "description"}),
                actor: Some(actor.to_string()),
            });
        }
        if let Some(ref new_tags) = updates.tags
            && new_tags != &arc.tags
        {
            history.push(HistoryEntry {
                ts: now,
                kind: "updated".into(),
                payload: serde_json::json!({"field": "tags"}),
                actor: Some(actor.to_string()),
            });
        }
        if let Some(ref new_refs) = updates.context_refs
            && new_refs != &arc.context_refs
        {
            history.push(HistoryEntry {
                ts: now,
                kind: "updated".into(),
                payload: serde_json::json!({"field": "context_refs"}),
                actor: Some(actor.to_string()),
            });
        }

        let new_title = updates.title.as_deref().unwrap_or(&arc.title);
        let new_desc = updates.description.as_deref().unwrap_or(&arc.description);
        let new_status = updates.status.as_deref().unwrap_or(&arc.status);
        let new_tags = updates.tags.as_deref().unwrap_or(&arc.tags);
        let new_refs = updates.context_refs.as_deref().unwrap_or(&arc.context_refs);
        let history_json = serde_json::to_string(&history)?;

        conn.execute(
            "UPDATE arcs SET \
                title=?1, description=?2, status=?3, tags=?4, \
                context_refs=?5, history=?6, updated_at=?7 \
            WHERE id=?8",
            rusqlite::params![
                new_title,
                new_desc,
                new_status,
                new_tags,
                new_refs,
                history_json,
                now,
                arc.id.as_bytes().as_slice(),
            ],
        )?;

        get_arc_by_uuid(&conn, &arc.id)?.ok_or_else(|| YojanaError::NotFound("updated arc".into()))
    }

    pub fn validate_task_arc(&self, arc_id: &Uuid, arc_phase: &str) -> Result<(), YojanaError> {
        let conn = self.conn.lock();
        let arc = get_arc_by_uuid(&conn, arc_id)?
            .ok_or_else(|| YojanaError::NotFound(format!("arc '{arc_id}'")))?;
        let phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases)?;
        let valid = phases
            .iter()
            .any(|p| p.get("name").and_then(|n| n.as_str()) == Some(arc_phase));
        if !valid {
            let names: Vec<&str> = phases
                .iter()
                .filter_map(|p| p.get("name").and_then(|n| n.as_str()))
                .collect();
            return Err(YojanaError::InvalidInput(format!(
                "unknown phase '{arc_phase}'; valid: {}",
                names.join(", ")
            )));
        }
        Ok(())
    }

    pub fn resolve_arc_id(&self, identifier: &str) -> Result<(Uuid, Uuid), YojanaError> {
        let conn = self.conn.lock();
        let arc = resolve_arc(&conn, identifier)?;
        Ok((arc.id, arc.project_id))
    }

    pub fn advance_arc_phase(
        &self,
        identifier: &str,
        phase_name: Option<&str>,
        skip: bool,
        note: Option<String>,
        actor: &str,
    ) -> Result<ArcRow, YojanaError> {
        let mut conn = self.conn.lock();
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(YojanaError::Db)?;

        let arc = resolve_arc(&tx, identifier)?;
        let now = chrono::Utc::now().timestamp_millis();
        let mut phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases)?;
        let mut history: Vec<HistoryEntry> = serde_json::from_str(&arc.history)?;

        let target_idx = if let Some(name) = phase_name {
            phases
                .iter()
                .position(|p| p.get("name").and_then(|n| n.as_str()) == Some(name))
                .ok_or_else(|| YojanaError::InvalidInput(format!("unknown phase '{name}'")))?
        } else {
            phases
                .iter()
                .position(|p| p.get("status").and_then(|s| s.as_str()) == Some("active"))
                .ok_or_else(|| YojanaError::InvalidInput("no active phase to advance".into()))?
        };

        let (target, following) = phases
            .split_at_mut(target_idx)
            .1
            .split_first_mut()
            .expect("invariant: target_idx came from phases.iter().position()");

        let target_status = target
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("pending");
        if target_status != "active" {
            return Err(YojanaError::InvalidInput(format!(
                "cannot advance phase '{}': status is '{}', expected 'active'",
                target.get("name").and_then(|n| n.as_str()).unwrap_or("?"),
                target_status
            )));
        }

        let phase_name_str = target
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("?")
            .to_string();

        let new_status = if skip { "skipped" } else { "completed" };
        set_field(
            target,
            "status",
            serde_json::Value::String(new_status.into()),
        );

        if let Some(next) = following
            .iter_mut()
            .find(|p| p.get("status").and_then(|s| s.as_str()) == Some("pending"))
        {
            set_field(next, "status", serde_json::Value::String("active".into()));
        }
        let mut payload = serde_json::json!({
            "phase": phase_name_str,
            "from": "active",
            "to": new_status,
        });
        if let Some(ref n) = note {
            set_field(&mut payload, "note", serde_json::Value::String(n.clone()));
        }
        history.push(HistoryEntry {
            ts: now,
            kind: "phase_advanced".into(),
            payload,
            actor: Some(actor.to_string()),
        });

        let phases_json = serde_json::to_string(&phases)?;
        let history_json = serde_json::to_string(&history)?;

        tx.execute(
            "UPDATE arcs SET phases=?1, history=?2, updated_at=?3 WHERE id=?4",
            rusqlite::params![phases_json, history_json, now, arc.id.as_bytes().as_slice(),],
        )?;

        let result = get_arc_by_uuid(&tx, &arc.id)?
            .ok_or_else(|| YojanaError::NotFound("advanced arc".into()))?;
        tx.commit().map_err(YojanaError::Db)?;
        Ok(result)
    }

    pub fn revert_arc_phase(
        &self,
        identifier: &str,
        phase_name: &str,
        note: Option<String>,
        actor: &str,
    ) -> Result<ArcRow, YojanaError> {
        let mut conn = self.conn.lock();
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(YojanaError::Db)?;

        let arc = resolve_arc(&tx, identifier)?;
        let now = chrono::Utc::now().timestamp_millis();
        let mut phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases)?;
        let mut history: Vec<HistoryEntry> = serde_json::from_str(&arc.history)?;

        let idx = phases
            .iter()
            .position(|p| p.get("name").and_then(|n| n.as_str()) == Some(phase_name))
            .ok_or_else(|| YojanaError::InvalidInput(format!("unknown phase '{phase_name}'")))?;

        let phase = phases
            .get_mut(idx)
            .expect("invariant: idx came from phases.iter().position()");

        let current_status = phase
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("pending");
        if current_status != "completed" {
            return Err(YojanaError::InvalidInput(format!(
                "cannot revert phase '{phase_name}': status is '{current_status}', expected 'completed'"
            )));
        }

        set_field(phase, "status", serde_json::Value::String("active".into()));

        let mut payload = serde_json::json!({
            "phase": phase_name,
            "from": "completed",
            "to": "active",
        });
        if let Some(ref n) = note {
            set_field(&mut payload, "note", serde_json::Value::String(n.clone()));
        }
        history.push(HistoryEntry {
            ts: now,
            kind: "phase_reverted".into(),
            payload,
            actor: Some(actor.to_string()),
        });

        let phases_json = serde_json::to_string(&phases)?;
        let history_json = serde_json::to_string(&history)?;

        tx.execute(
            "UPDATE arcs SET phases=?1, history=?2, updated_at=?3 WHERE id=?4",
            rusqlite::params![phases_json, history_json, now, arc.id.as_bytes().as_slice(),],
        )?;

        let result = get_arc_by_uuid(&tx, &arc.id)?
            .ok_or_else(|| YojanaError::NotFound("reverted arc".into()))?;
        tx.commit().map_err(YojanaError::Db)?;
        Ok(result)
    }

    pub fn try_auto_advance_phase(&self, task_id: &Uuid, actor: &str) -> Result<bool, YojanaError> {
        let mut conn = self.conn.lock();

        let task = get_task_by_uuid(&conn, task_id)?
            .ok_or_else(|| YojanaError::NotFound(format!("task '{task_id}'")))?;

        let (arc_id, arc_phase) = match (&task.arc_id, &task.arc_phase) {
            (Some(id), Some(phase)) => (*id, phase.clone()),
            _ => return Ok(false),
        };

        if !TERMINAL_STATUSES.contains(&task.status.as_str()) {
            return Ok(false);
        }

        let arc = get_arc_by_uuid(&conn, &arc_id)?
            .ok_or_else(|| YojanaError::NotFound(format!("arc '{arc_id}'")))?;

        if arc.status != "active" {
            return Ok(false);
        }

        let phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases)?;
        let Some((phase_idx, phase)) = phases
            .iter()
            .enumerate()
            .find(|(_, p)| p.get("name").and_then(|n| n.as_str()) == Some(&arc_phase))
        else {
            return Ok(false);
        };

        let gate = phase.get("gate").and_then(|g| g.as_str()).unwrap_or("auto");
        if gate != "auto" {
            return Ok(false);
        }

        let phase_status = phase
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("pending");
        if phase_status != "active" {
            return Ok(false);
        }

        let all_terminal = {
            let mut stmt = conn.prepare(&format!(
                "{TASK_SELECT} WHERE t.arc_id = ?1 AND t.arc_phase = ?2"
            ))?;
            let sibling_tasks: Vec<TaskRow> = stmt
                .query_map(
                    rusqlite::params![arc_id.as_bytes().as_slice(), &arc_phase],
                    map_task_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;

            if sibling_tasks.is_empty() {
                return Ok(false);
            }

            sibling_tasks
                .iter()
                .all(|t| TERMINAL_STATUSES.contains(&t.status.as_str()))
        };
        if !all_terminal {
            return Ok(false);
        }

        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(YojanaError::Db)?;

        let now = chrono::Utc::now().timestamp_millis();
        let mut phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases)?;
        let mut history: Vec<HistoryEntry> = serde_json::from_str(&arc.history)?;

        let (current, following) =
            phases.split_at_mut(phase_idx).1.split_first_mut().expect(
                "invariant: phase_idx came from an enumerate().find() over the same phases",
            );

        set_field(
            current,
            "status",
            serde_json::Value::String("completed".into()),
        );

        if let Some(next) = following
            .iter_mut()
            .find(|p| p.get("status").and_then(|s| s.as_str()) == Some("pending"))
        {
            set_field(next, "status", serde_json::Value::String("active".into()));
        }

        history.push(HistoryEntry {
            ts: now,
            kind: "phase_auto_advanced".into(),
            payload: serde_json::json!({
                "phase": arc_phase,
                "from": "active",
                "to": "completed",
                "trigger_task": format!("{}/{}", task.project_slug, task.sequence_number),
            }),
            actor: Some(actor.to_string()),
        });

        let phases_json = serde_json::to_string(&phases)?;
        let history_json = serde_json::to_string(&history)?;

        tx.execute(
            "UPDATE arcs SET phases=?1, history=?2, updated_at=?3 WHERE id=?4",
            rusqlite::params![phases_json, history_json, now, arc.id.as_bytes().as_slice()],
        )?;
        tx.commit().map_err(YojanaError::Db)?;

        Ok(true)
    }

    // --- Query methods ---

    pub fn list_tasks(&self, filter: &TaskQueryFilter) -> Result<Vec<TaskRow>, YojanaError> {
        let conn = self.conn.lock();
        let mut sql = String::from(TASK_SELECT);
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref project_ids) = filter.project_ids {
            if let [only] = project_ids.as_slice() {
                params.push(Box::new(only.as_bytes().to_vec()));
                conditions.push(format!("t.project_id = ?{}", params.len()));
            } else if !project_ids.is_empty() {
                let placeholders: Vec<String> = project_ids
                    .iter()
                    .map(|pid| {
                        params.push(Box::new(pid.as_bytes().to_vec()));
                        format!("?{}", params.len())
                    })
                    .collect();
                conditions.push(format!("t.project_id IN ({})", placeholders.join(", ")));
            }
        }
        if let Some(ref status) = filter.status {
            params.push(Box::new(status.clone()));
            conditions.push(format!("t.status = ?{}", params.len()));
        }
        if let Some(ref category) = filter.category {
            params.push(Box::new(category.clone()));
            conditions.push(format!("t.category = ?{}", params.len()));
        }
        if let Some(ref slice_type) = filter.slice_type {
            params.push(Box::new(slice_type.clone()));
            conditions.push(format!("t.slice_type = ?{}", params.len()));
        }
        if let Some(ref tag) = filter.tag {
            params.push(Box::new(tag.clone()));
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM json_each(t.tags) WHERE json_each.value = ?{})",
                params.len()
            ));
        }
        if let Some(ref arc_id) = filter.arc_id {
            params.push(Box::new(arc_id.as_bytes().to_vec()));
            conditions.push(format!("t.arc_id = ?{}", params.len()));
        }
        if let Some(cutoff) = filter.include_terminal_after {
            let terminal_list = TERMINAL_STATUSES
                .iter()
                .map(|s| format!("'{s}'"))
                .collect::<Vec<_>>()
                .join(", ");
            params.push(Box::new(cutoff));
            conditions.push(format!(
                "(t.status NOT IN ({terminal_list}) OR t.completed_at >= ?{})",
                params.len()
            ));
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY t.updated_at DESC");

        let limit = filter.limit.unwrap_or(DEFAULT_PAGE_LIMIT);
        params.push(Box::new(limit));
        sql.push_str(&format!(" LIMIT ?{}", params.len()));

        if let Some(offset) = filter.offset {
            params.push(Box::new(offset));
            sql.push_str(&format!(" OFFSET ?{}", params.len()));
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), map_task_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_depends_on_with_status(&self) -> Result<Vec<(Uuid, Uuid, String)>, YojanaError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT e.source_task_id, e.target_task_id, t.status \
             FROM task_edges e JOIN tasks t ON e.target_task_id = t.id \
             WHERE e.edge_type = 'depends_on'",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let src = uuid_from_blob(row, "source_task_id")?;
                let tgt = uuid_from_blob(row, "target_task_id")?;
                let status: String = row.get("status")?;
                Ok((src, tgt, status))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Map a set of task UUIDs to their human ids ("{project_slug}/{seq}").
    /// Used to render blocker references without leaking raw UUIDs.
    pub fn task_human_ids(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, String>, YojanaError> {
        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let conn = self.conn.lock();
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT t.id, p.slug, t.sequence_number \
             FROM tasks t JOIN projects p ON t.project_id = p.id \
             WHERE t.id IN ({})",
            placeholders.join(", ")
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<Vec<u8>> = ids.iter().map(|id| id.as_bytes().to_vec()).collect();
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|p| p as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let id = uuid_from_blob(row, "id")?;
                let slug: String = row.get("slug")?;
                let seq: i64 = row.get("sequence_number")?;
                Ok((id, format!("{slug}/{seq}")))
            })?
            .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
        Ok(rows)
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
        if source_task_id == target_task_id {
            return Err(YojanaError::InvalidInput("self-edges not allowed".into()));
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

        get_edge_by_id(&conn, &id)?.ok_or_else(|| YojanaError::NotFound("just-created edge".into()))
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
            .query_map(
                rusqlite::params![task_id.as_bytes().as_slice()],
                map_edge_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_edges_by_type_for_tasks(
        &self,
        task_ids: &[Uuid],
        edge_type: &str,
    ) -> Result<Vec<EdgeRow>, YojanaError> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock();
        let placeholders: Vec<String> = (1..=task_ids.len())
            .map(|i| format!("?{}", i + 1))
            .collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT id, source_task_id, target_task_id, edge_type, note, created_at \
             FROM task_edges WHERE edge_type = ?1 \
             AND source_task_id IN ({in_clause}) \
             ORDER BY created_at"
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params.push(Box::new(edge_type.to_string()));
        for id in task_ids {
            params.push(Box::new(id.as_bytes().to_vec()));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), map_edge_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- Conversation methods ---

    pub fn append_conversation_message(
        &self,
        task_id: &Uuid,
        text: &str,
        author: Option<&str>,
    ) -> Result<serde_json::Value, YojanaError> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp_millis();

        let message = serde_json::json!({
            "ts": now,
            "text": text,
            "author": author.unwrap_or("user"),
        });

        let existing: Option<(Vec<u8>, String)> = conn
            .prepare("SELECT id, messages FROM task_conversations WHERE task_id = ?1")?
            .query_row(rusqlite::params![task_id.as_bytes().as_slice()], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()?;

        if let Some((id_bytes, messages_json)) = existing {
            let mut messages: Vec<serde_json::Value> = match serde_json::from_str(&messages_json) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("corrupt JSON in conversation messages: {e}");
                    Vec::new()
                }
            };
            messages.push(message.clone());
            let updated_json = serde_json::to_string(&messages)?;
            conn.execute(
                "UPDATE task_conversations SET messages = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![updated_json, now, id_bytes],
            )?;
        } else {
            let id = Uuid::now_v7();
            let messages_json = serde_json::to_string(&vec![&message])?;
            conn.execute(
                "INSERT INTO task_conversations (id, task_id, messages, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                rusqlite::params![
                    id.as_bytes().as_slice(),
                    task_id.as_bytes().as_slice(),
                    messages_json,
                    now,
                ],
            )?;
        }

        Ok(message)
    }

    pub fn get_conversation_messages(
        &self,
        task_id: &Uuid,
    ) -> Result<Vec<serde_json::Value>, YojanaError> {
        let conn = self.conn.lock();
        let messages_json: Option<String> = conn
            .prepare("SELECT messages FROM task_conversations WHERE task_id = ?1")?
            .query_row(rusqlite::params![task_id.as_bytes().as_slice()], |row| {
                row.get(0)
            })
            .optional()?;

        match messages_json {
            Some(json) => match serde_json::from_str(&json) {
                Ok(v) => Ok(v),
                Err(e) => {
                    tracing::warn!("corrupt JSON in conversation messages: {e}");
                    Ok(Vec::new())
                }
            },
            None => Ok(Vec::new()),
        }
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
        let p = db
            .create_project("test-proj", "Test Project", "A test", None, "test")
            .unwrap();
        assert_eq!(p.slug, "test-proj");
        assert_eq!(p.title, "Test Project");
        assert_eq!(p.description, "A test");
        assert_eq!(p.status, "active");

        let by_id = db
            .get_project(Some(&p.id.to_string()), None)
            .unwrap()
            .unwrap();
        assert_eq!(by_id.slug, "test-proj");

        let by_slug = db.get_project(None, Some("test-proj")).unwrap().unwrap();
        assert_eq!(by_slug.id, p.id);
    }

    #[test]
    fn slug_uniqueness() {
        let db = test_db();
        db.create_project("dupe", "First", "", None, "test")
            .unwrap();
        let err = db
            .create_project("dupe", "Second", "", None, "test")
            .unwrap_err();
        assert!(matches!(err, YojanaError::Conflict(_)));
    }

    #[test]
    fn list_with_status_filter() {
        let db = test_db();
        db.create_project("a", "A", "", None, "test").unwrap();
        db.create_project("b", "B", "", None, "test").unwrap();

        let all = db.list_projects(None, None, None, None).unwrap();
        assert_eq!(all.len(), 2);

        let active = db.list_projects(Some("active"), None, None, None).unwrap();
        assert_eq!(active.len(), 2);

        let paused = db.list_projects(Some("paused"), None, None, None).unwrap();
        assert_eq!(paused.len(), 0);
    }

    #[test]
    fn update_records_history() {
        let db = test_db();
        let p = db
            .create_project("test", "Original", "desc", None, "test")
            .unwrap();

        let updated = db
            .update_project(
                Some(&p.id.to_string()),
                None,
                ProjectUpdates {
                    title: Some("New Title".into()),
                    ..Default::default()
                },
                "test",
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
        db.create_project("test", "Test", "", None, "test").unwrap();

        let updated = db
            .update_project(
                None,
                Some("test"),
                ProjectUpdates {
                    status: Some("paused".into()),
                    ..Default::default()
                },
                "test",
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
        let p = db.get_project(None, Some(project_slug)).unwrap().unwrap();
        db.create_task(
            CreateTaskParams {
                project_id: p.id,
                project_slug: p.slug,
                title: title.to_string(),
                description: String::new(),
                category: None,
                status: None,
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
                arc_id: None,
                arc_phase: None,
            },
            "test",
        )
        .unwrap()
    }

    #[test]
    fn create_task_with_sequence_numbers() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();

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
    fn create_task_respects_status() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();

        let t = db
            .create_task(
                CreateTaskParams {
                    project_id: p.id,
                    project_slug: p.slug.clone(),
                    title: "With status".into(),
                    description: String::new(),
                    category: None,
                    status: Some("ready-for-agent".into()),
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
                    arc_id: None,
                    arc_phase: None,
                },
                "test",
            )
            .unwrap();
        assert_eq!(t.status, "ready-for-agent");

        let t2 = create_test_task(&db, "proj", "No status");
        assert_eq!(t2.status, "needs-triage");

        let err = db.create_task(
            CreateTaskParams {
                project_id: p.id,
                project_slug: p.slug,
                title: "Bad status".into(),
                description: String::new(),
                category: None,
                status: Some("bogus".into()),
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
                arc_id: None,
                arc_phase: None,
            },
            "test",
        );
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("unknown status"));
    }

    #[test]
    fn sequence_numbers_are_per_project() {
        let db = test_db();
        db.create_project("alpha", "Alpha", "", None, "test")
            .unwrap();
        db.create_project("beta", "Beta", "", None, "test").unwrap();

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
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Task");

        let fetched = db.get_task(&t.id.to_string()).unwrap().unwrap();
        assert_eq!(fetched.title, "Task");
        assert_eq!(fetched.project_slug, "proj");
    }

    #[test]
    fn get_task_by_slug_seq() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        create_test_task(&db, "proj", "First");
        let t2 = create_test_task(&db, "proj", "Second");

        let fetched = db.get_task("proj/2").unwrap().unwrap();
        assert_eq!(fetched.id, t2.id);
        assert_eq!(fetched.title, "Second");
    }

    #[test]
    fn update_task_partial() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Original");

        let updated = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    title: Some("Changed".into()),
                    category: Some(Some("bug".into())),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();

        assert_eq!(updated.title, "Changed");
        assert_eq!(updated.category.as_deref(), Some("bug"));
        assert_eq!(updated.status, "needs-triage"); // unchanged
    }

    #[test]
    fn json_round_trip() {
        let db = test_db();
        let p = db
            .create_project("proj", "Project", "", None, "test")
            .unwrap();

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
            .create_task(
                CreateTaskParams {
                    project_id: p.id,
                    project_slug: p.slug,
                    title: "JSON test".into(),
                    description: String::new(),
                    category: Some("enhancement".into()),
                    status: None,
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
                    arc_id: None,
                    arc_phase: None,
                },
                "test",
            )
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
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
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
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Task");
        assert_eq!(t.status, "needs-triage");

        let updated = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    status: Some("ready-for-agent".into()),
                    ..Default::default()
                },
                "test",
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
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Task");

        let err = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    status: Some("done".into()),
                    ..Default::default()
                },
                "test",
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid transition"));
    }

    #[test]
    fn noop_status_update_skips_validation() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Task");

        let updated = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    status: Some("needs-triage".into()),
                    ..Default::default()
                },
                "test",
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
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Task");

        let updated = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    title: Some("New title".into()),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();
        assert_eq!(updated.title, "New title");
        assert_eq!(updated.status, "needs-triage");
    }

    #[test]
    fn done_is_terminal_except_reopen() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Task");
        let id = t.id.to_string();

        db.update_task(
            &id,
            TaskUpdates {
                status: Some("ready-for-agent".into()),
                ..Default::default()
            },
            "test",
        )
        .unwrap();
        db.update_task(
            &id,
            TaskUpdates {
                status: Some("in-progress".into()),
                ..Default::default()
            },
            "test",
        )
        .unwrap();
        db.update_task(
            &id,
            TaskUpdates {
                status: Some("done".into()),
                ..Default::default()
            },
            "test",
        )
        .unwrap();

        let err = db
            .update_task(
                &id,
                TaskUpdates {
                    status: Some("in-progress".into()),
                    ..Default::default()
                },
                "test",
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid transition"));

        let reopened = db
            .update_task(
                &id,
                TaskUpdates {
                    status: Some("needs-triage".into()),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();
        assert_eq!(reopened.status, "needs-triage");
    }

    // --- Slice 04: edge CRUD ---

    #[test]
    fn create_and_list_edges() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
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
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
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
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();
        let err = db
            .create_edge(t1.id, t2.id, "depends_on", None)
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn same_pair_different_types_allowed() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
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
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();
        let err = db
            .create_edge(t2.id, t1.id, "depends_on", None)
            .unwrap_err();
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn cycle_detection_rejects_multi_hop() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");
        let t3 = create_test_task(&db, "proj", "Task 3");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();
        db.create_edge(t2.id, t3.id, "depends_on", None).unwrap();
        let err = db
            .create_edge(t3.id, t1.id, "depends_on", None)
            .unwrap_err();
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn non_dependency_edges_skip_cycle_check() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        db.create_edge(t1.id, t2.id, "relates_to", None).unwrap();
        db.create_edge(t2.id, t1.id, "relates_to", None).unwrap();
    }

    #[test]
    fn invalid_edge_type_rejected() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "Task 1");
        let t2 = create_test_task(&db, "proj", "Task 2");

        let err = db.create_edge(t1.id, t2.id, "blocks", None).unwrap_err();
        assert!(err.to_string().contains("invalid edge_type"));
    }

    #[test]
    fn cross_project_edges() {
        let db = test_db();
        db.create_project("alpha", "Alpha", "", None, "test")
            .unwrap();
        db.create_project("beta", "Beta", "", None, "test").unwrap();
        let t1 = create_test_task(&db, "alpha", "Task A");
        let t2 = create_test_task(&db, "beta", "Task B");

        let edge = db.create_edge(t1.id, t2.id, "motivated_by", None).unwrap();
        assert_eq!(edge.source_task_id, t1.id);
        assert_eq!(edge.target_task_id, t2.id);
    }

    #[test]
    fn cascade_delete_task_removes_edges() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
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

    // --- Slice 05: query + ready detection ---

    fn advance_to(db: &Db, id: &str, statuses: &[&str]) {
        for s in statuses {
            db.update_task(
                id,
                TaskUpdates {
                    status: Some((*s).into()),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();
        }
    }

    #[test]
    fn list_tasks_no_filter() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        create_test_task(&db, "proj", "A");
        create_test_task(&db, "proj", "B");

        let tasks = db.list_tasks(&TaskQueryFilter::default()).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn list_tasks_filter_by_status() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "A");
        create_test_task(&db, "proj", "B");

        advance_to(&db, &t1.id.to_string(), &["ready-for-agent"]);

        let tasks = db
            .list_tasks(&TaskQueryFilter {
                status: Some("ready-for-agent".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "A");
    }

    #[test]
    fn list_tasks_filter_by_project() {
        let db = test_db();
        let p1 = db
            .create_project("alpha", "Alpha", "", None, "test")
            .unwrap();
        db.create_project("beta", "Beta", "", None, "test").unwrap();
        create_test_task(&db, "alpha", "A");
        create_test_task(&db, "beta", "B");

        let tasks = db
            .list_tasks(&TaskQueryFilter {
                project_ids: Some(vec![p1.id]),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "A");
    }

    #[test]
    fn list_tasks_filter_by_tag() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();
        db.create_task(
            CreateTaskParams {
                project_id: p.id,
                project_slug: p.slug.clone(),
                title: "Tagged".into(),
                description: String::new(),
                category: None,
                status: None,
                slice_type: None,
                acceptance_criteria: "[]".into(),
                decisions: "[]".into(),
                context_refs: "[]".into(),
                files: "[]".into(),
                tags: serde_json::to_string(&vec!["infra"]).unwrap(),
                implementation_plan: None,
                execution_record: None,
                reproduction: None,
                root_cause: None,
                arc_id: None,
                arc_phase: None,
            },
            "test",
        )
        .unwrap();
        create_test_task(&db, "proj", "Untagged");

        let tasks = db
            .list_tasks(&TaskQueryFilter {
                tag: Some("infra".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Tagged");
    }

    #[test]
    fn cross_project_query() {
        let db = test_db();
        db.create_project("alpha", "Alpha", "", None, "test")
            .unwrap();
        db.create_project("beta", "Beta", "", None, "test").unwrap();
        create_test_task(&db, "alpha", "A");
        create_test_task(&db, "beta", "B");

        let tasks = db.list_tasks(&TaskQueryFilter::default()).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn depends_on_with_status() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "A");
        let t2 = create_test_task(&db, "proj", "B");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();

        let deps = db.list_depends_on_with_status().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].0, t1.id);
        assert_eq!(deps[0].1, t2.id);
        assert_eq!(deps[0].2, "needs-triage");
    }

    #[test]
    fn ready_detection_no_deps() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "A");
        let id = t.id.to_string();
        advance_to(&db, &id, &["ready-for-agent"]);

        let deps = db.list_depends_on_with_status().unwrap();
        assert!(crate::graph::is_ready(t.id, &deps));
    }

    #[test]
    fn ready_detection_deps_done() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "Depends");
        let t2 = create_test_task(&db, "proj", "Dependency");

        let id1 = t1.id.to_string();
        let id2 = t2.id.to_string();

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();

        advance_to(&db, &id1, &["ready-for-agent"]);
        advance_to(&db, &id2, &["ready-for-agent", "in-progress", "done"]);

        let deps = db.list_depends_on_with_status().unwrap();
        assert!(crate::graph::is_ready(t1.id, &deps));
    }

    #[test]
    fn ready_detection_deps_not_done() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "Depends");
        let t2 = create_test_task(&db, "proj", "Dependency");

        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();

        let deps = db.list_depends_on_with_status().unwrap();
        assert!(!crate::graph::is_ready(t1.id, &deps));
    }

    #[test]
    fn ready_detection_diamond() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let leaf = create_test_task(&db, "proj", "Leaf");
        let mid1 = create_test_task(&db, "proj", "Mid1");
        let mid2 = create_test_task(&db, "proj", "Mid2");

        db.create_edge(leaf.id, mid1.id, "depends_on", None)
            .unwrap();
        db.create_edge(leaf.id, mid2.id, "depends_on", None)
            .unwrap();

        let mid1_id = mid1.id.to_string();
        let mid2_id = mid2.id.to_string();
        advance_to(&db, &mid1_id, &["ready-for-agent", "in-progress", "done"]);

        let deps = db.list_depends_on_with_status().unwrap();
        assert!(!crate::graph::is_ready(leaf.id, &deps));

        advance_to(&db, &mid2_id, &["ready-for-agent", "in-progress", "done"]);

        let deps = db.list_depends_on_with_status().unwrap();
        assert!(crate::graph::is_ready(leaf.id, &deps));
    }

    // --- Slice 06: conversations ---

    #[test]
    fn append_and_get_conversation() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Task");

        let msgs = db.get_conversation_messages(&t.id).unwrap();
        assert!(msgs.is_empty());

        let m1 = db
            .append_conversation_message(&t.id, "hello", Some("agent"))
            .unwrap();
        assert_eq!(m1["text"], "hello");
        assert_eq!(m1["author"], "agent");

        let m2 = db
            .append_conversation_message(&t.id, "world", None)
            .unwrap();
        assert_eq!(m2["text"], "world");
        assert_eq!(m2["author"], "user");

        let msgs = db.get_conversation_messages(&t.id).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["text"], "hello");
        assert_eq!(msgs[1]["text"], "world");
    }

    #[test]
    fn conversation_cascade_on_task_delete() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Task");
        db.append_conversation_message(&t.id, "msg", None).unwrap();

        {
            let conn = db.conn.lock();
            conn.execute(
                "DELETE FROM tasks WHERE id = ?1",
                rusqlite::params![t.id.as_bytes().as_slice()],
            )
            .unwrap();
        }

        let msgs = db.get_conversation_messages(&t.id).unwrap();
        assert!(msgs.is_empty());
    }

    // --- Slice 06: context integration ---

    #[test]
    fn context_summary_integration() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "Main Task");
        let t2 = create_test_task(&db, "proj", "Dep");
        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();

        let edges = db.list_edges_for_task(&t1.id).unwrap();
        let bundle = crate::context::summary(&t1, &edges);

        assert_eq!(bundle.human_id, "proj/1");
        assert_eq!(bundle.title, "Main Task");
        assert_eq!(bundle.edge_counts.get("depends_on_out"), Some(&1));
    }

    #[test]
    fn context_working_integration() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t1 = create_test_task(&db, "proj", "Main");
        let t2 = create_test_task(&db, "proj", "Neighbor");
        db.create_edge(t1.id, t2.id, "depends_on", None).unwrap();
        db.append_conversation_message(&t1.id, "started work", Some("agent"))
            .unwrap();

        let edges = db.list_edges_for_task(&t1.id).unwrap();
        let nids = crate::context::neighbor_ids(t1.id, &edges);
        assert_eq!(nids.len(), 1);

        let mut neighbors_with_edges = Vec::new();
        for nid in &nids {
            let ntask = db.get_task(&nid.to_string()).unwrap().unwrap();
            let nedges = db.list_edges_for_task(&ntask.id).unwrap();
            neighbors_with_edges.push((ntask, nedges));
        }

        let messages = db.get_conversation_messages(&t1.id).unwrap();
        let bundle = crate::context::working(&t1, &neighbors_with_edges, &messages, 10, None);

        assert_eq!(bundle.human_id, "proj/1");
        assert_eq!(bundle.neighbors.len(), 1);
        assert_eq!(bundle.neighbors[0].human_id, "proj/2");
        assert_eq!(bundle.recent_messages.len(), 1);
        assert_eq!(bundle.recent_messages[0]["text"], "started work");
    }

    #[test]
    fn tag_filter_no_wildcard_false_positive() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();
        db.create_task(
            CreateTaskParams {
                project_id: p.id,
                project_slug: p.slug.clone(),
                title: "Has a%b tag".into(),
                description: String::new(),
                category: None,
                status: None,
                slice_type: None,
                acceptance_criteria: "[]".into(),
                decisions: "[]".into(),
                context_refs: "[]".into(),
                files: "[]".into(),
                tags: serde_json::to_string(&vec!["a%b"]).unwrap(),
                implementation_plan: None,
                execution_record: None,
                reproduction: None,
                root_cause: None,
                arc_id: None,
                arc_phase: None,
            },
            "test",
        )
        .unwrap();

        let matches = db
            .list_tasks(&TaskQueryFilter {
                tag: Some("a%b".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(matches.len(), 1);

        let no_match = db
            .list_tasks(&TaskQueryFilter {
                tag: Some("axb".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(no_match.len(), 0, "LIKE wildcard should not match 'axb'");
    }

    #[test]
    fn clear_nullable_fields_to_null() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();
        let t = db
            .create_task(
                CreateTaskParams {
                    project_id: p.id,
                    project_slug: p.slug.clone(),
                    title: "Clearable".into(),
                    description: String::new(),
                    category: Some("bug".into()),
                    status: None,
                    slice_type: Some("AFK".into()),
                    acceptance_criteria: "[]".into(),
                    decisions: "[]".into(),
                    context_refs: "[]".into(),
                    files: "[]".into(),
                    tags: "[]".into(),
                    implementation_plan: Some("plan".into()),
                    execution_record: None,
                    reproduction: None,
                    root_cause: None,
                    arc_id: None,
                    arc_phase: None,
                },
                "test",
            )
            .unwrap();
        assert_eq!(t.category.as_deref(), Some("bug"));

        let cleared = db
            .update_task(
                &t.id.to_string(),
                TaskUpdates {
                    category: Some(None),
                    slice_type: Some(None),
                    implementation_plan: Some(None),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();
        assert_eq!(cleared.category, None);
        assert_eq!(cleared.slice_type, None);
        assert_eq!(cleared.implementation_plan, None);
    }

    #[test]
    fn project_status_validation_rejects_typo() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let err = db
            .update_project(
                None,
                Some("proj"),
                ProjectUpdates {
                    status: Some("actve".into()),
                    ..Default::default()
                },
                "test",
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid project status"));
    }

    #[test]
    fn project_status_validation_accepts_valid() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db
            .update_project(
                None,
                Some("proj"),
                ProjectUpdates {
                    status: Some("paused".into()),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();
        assert_eq!(p.status, "paused");
    }

    #[test]
    fn self_edge_rejected() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Solo");
        let err = db.create_edge(t.id, t.id, "relates_to", None).unwrap_err();
        assert!(err.to_string().contains("self-edges not allowed"));
    }

    #[test]
    fn migration_table_populated() {
        let db = test_db();
        let conn = db.conn.lock();
        let count: i64 = conn
            .prepare("SELECT COUNT(*) FROM _yojana_migrations")
            .unwrap()
            .query_row([], |row| row.get(0))
            .unwrap();
        assert!(
            count >= 5,
            "expected at least 5 migrations applied, got {count}"
        );
    }

    /// The 0012 backfill has to run against rows that already exist, so seed the
    /// legacy shapes, unrecord the migration, and re-run it.
    #[test]
    fn acceptance_criteria_backfill_normalizes_string_form() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let string_form = create_test_task(&db, "proj", "String form");
        let object_form = create_test_task(&db, "proj", "Object form");
        let empty = create_test_task(&db, "proj", "Empty");
        let mixed = create_test_task(&db, "proj", "Mixed form");

        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE tasks SET acceptance_criteria = ?1 WHERE id = ?2",
                rusqlite::params![
                    r#"["first","second"]"#,
                    string_form.id.as_bytes().as_slice()
                ],
            )
            .unwrap();
            conn.execute(
                "UPDATE tasks SET acceptance_criteria = ?1 WHERE id = ?2",
                rusqlite::params![
                    r#"[{"text":"kept","done":true}]"#,
                    object_form.id.as_bytes().as_slice()
                ],
            )
            .unwrap();
            conn.execute(
                "UPDATE tasks SET acceptance_criteria = ?1 WHERE id = ?2",
                rusqlite::params![
                    r#"["loose",{"text":"typed","done":true}]"#,
                    mixed.id.as_bytes().as_slice()
                ],
            )
            .unwrap();
            conn.execute(
                "DELETE FROM _yojana_migrations WHERE name = '0012_normalize_acceptance_criteria'",
                [],
            )
            .unwrap();
        }
        db.run_migrations().unwrap();

        let migrated = db.get_task(&string_form.id.to_string()).unwrap().unwrap();
        assert_eq!(
            migrated.acceptance_criteria,
            r#"[{"text":"first","done":false},{"text":"second","done":false}]"#,
            "string form should become object form with order preserved"
        );

        let untouched = db.get_task(&object_form.id.to_string()).unwrap().unwrap();
        assert_eq!(
            untouched.acceptance_criteria, r#"[{"text":"kept","done":true}]"#,
            "object form should be left alone"
        );

        let still_empty = db.get_task(&empty.id.to_string()).unwrap().unwrap();
        assert_eq!(still_empty.acceptance_criteria, "[]");

        let mixed = db.get_task(&mixed.id.to_string()).unwrap().unwrap();
        assert_eq!(
            mixed.acceptance_criteria,
            r#"[{"text":"loose","done":false},{"text":"typed","done":true}]"#,
            "a mixed array should normalize the strings and keep the objects"
        );
    }

    /// 0013 has to cope with every shape the old untyped `items: true` schema
    /// let through, including the ones 0012 never selected.
    #[test]
    fn acceptance_criteria_0013_canonicalizes_every_shape() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();

        // (seed value, expected value after 0013, quarantined?)
        let cases: Vec<(&str, &str, bool)> = vec![
            (r#"["str"]"#, r#"[{"text":"str","done":false}]"#, false),
            (
                r#"[{"text":"a","done":true}]"#,
                r#"[{"text":"a","done":true}]"#,
                false,
            ),
            (r#"[{"text":"b"}]"#, r#"[{"text":"b","done":false}]"#, false),
            ("[]", "[]", false),
            (
                r#"["keep",{"text":"order","done":false},"third"]"#,
                r#"[{"text":"keep","done":false},{"text":"order","done":false},{"text":"third","done":false}]"#,
                false,
            ),
            (
                r#"[true,7,null,["nested"],{"note":"no text"}]"#,
                r#"[{"text":"<unmigratable true: 1>","done":false},{"text":"<unmigratable integer: 7>","done":false},{"text":"<unmigratable null: null>","done":false},{"text":"<unmigratable array: [\"nested\"]>","done":false},{"text":"<unmigratable object: {\"note\":\"no text\"}>","done":false}]"#,
                true,
            ),
            (
                r#"{"text":"not an array"}"#,
                r#"[{"text":"<unmigratable column: {\"text\":\"not an array\"}>","done":false}]"#,
                true,
            ),
            (
                "not json at all",
                r#"[{"text":"<unmigratable column: not json at all>","done":false}]"#,
                true,
            ),
        ];

        let seeded: Vec<Uuid> = cases
            .iter()
            .enumerate()
            .map(|(i, (seed, _, _))| {
                let task = create_test_task(&db, "proj", &format!("Task {i}"));
                let conn = db.conn.lock();
                conn.execute(
                    "UPDATE tasks SET acceptance_criteria = ?1 WHERE id = ?2",
                    rusqlite::params![seed, task.id.as_bytes().as_slice()],
                )
                .unwrap();
                task.id
            })
            .collect();

        {
            let conn = db.conn.lock();
            conn.execute(
                "DELETE FROM _yojana_migrations WHERE name = '0013_canonical_acceptance_criteria'",
                [],
            )
            .unwrap();
            conn.execute("DELETE FROM acceptance_criteria_quarantine", [])
                .unwrap();
        }
        db.run_migrations().unwrap();

        for (id, (seed, expected, quarantined)) in seeded.iter().zip(cases.iter()) {
            let row = db.get_task(&id.to_string()).unwrap().unwrap();
            assert_eq!(
                row.acceptance_criteria, *expected,
                "seed {seed} did not canonicalize as expected"
            );

            let conn = db.conn.lock();
            let count: i64 = conn
                .prepare("SELECT COUNT(*) FROM acceptance_criteria_quarantine WHERE task_id = ?1")
                .unwrap()
                .query_row(rusqlite::params![id.as_bytes().as_slice()], |r| r.get(0))
                .unwrap();
            assert_eq!(
                count,
                i64::from(*quarantined),
                "seed {seed}: unexpected quarantine state"
            );
        }

        // Every element is now an object with a string text and a boolean done.
        let conn = db.conn.lock();
        let non_canonical: i64 = conn
            .prepare(
                "SELECT COUNT(*) FROM tasks t, json_each(t.acceptance_criteria) je \
                 WHERE je.type <> 'object' \
                    OR json_type(je.value, '$.text') <> 'text' \
                    OR json_type(je.value, '$.done') NOT IN ('true','false')",
            )
            .unwrap()
            .query_row([], |r| r.get(0))
            .unwrap();
        assert_eq!(non_canonical, 0, "criteria left in a non-canonical shape");
    }

    /// Re-applying 0013 must change nothing — including not re-quarantining.
    #[test]
    fn acceptance_criteria_0013_is_idempotent() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let task = create_test_task(&db, "proj", "Task");

        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE tasks SET acceptance_criteria = ?1 WHERE id = ?2",
                rusqlite::params![r#"["a",{"text":"b"},7]"#, task.id.as_bytes().as_slice()],
            )
            .unwrap();
            conn.execute(
                "DELETE FROM _yojana_migrations WHERE name = '0013_canonical_acceptance_criteria'",
                [],
            )
            .unwrap();
        }
        db.run_migrations().unwrap();
        let first = db.get_task(&task.id.to_string()).unwrap().unwrap();

        {
            let conn = db.conn.lock();
            conn.execute(
                "DELETE FROM _yojana_migrations WHERE name = '0013_canonical_acceptance_criteria'",
                [],
            )
            .unwrap();
        }
        db.run_migrations().unwrap();
        let second = db.get_task(&task.id.to_string()).unwrap().unwrap();

        assert_eq!(
            first.acceptance_criteria, second.acceptance_criteria,
            "second application changed the data"
        );

        let conn = db.conn.lock();
        let quarantined: i64 = conn
            .prepare("SELECT COUNT(*) FROM acceptance_criteria_quarantine")
            .unwrap()
            .query_row([], |r| r.get(0))
            .unwrap();
        assert_eq!(
            quarantined, 1,
            "re-run should not duplicate quarantine rows"
        );
    }

    #[test]
    fn in_progress_renamed_to_hyphen() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let t = create_test_task(&db, "proj", "Task");
        let id = t.id.to_string();
        advance_to(&db, &id, &["ready-for-agent", "in-progress"]);
        let fetched = db.get_task(&id).unwrap().unwrap();
        assert_eq!(fetched.status, "in-progress");
    }

    #[test]
    fn pagination_limits_results() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        for i in 0..5 {
            create_test_task(&db, "proj", &format!("Task {i}"));
        }

        let page = db
            .list_tasks(&TaskQueryFilter {
                limit: Some(2),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.len(), 2);

        let page2 = db
            .list_tasks(&TaskQueryFilter {
                limit: Some(2),
                offset: Some(2),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page2.len(), 2);
        assert_ne!(page[0].id, page2[0].id);
    }

    #[test]
    fn pagination_projects() {
        let db = test_db();
        for i in 0..5 {
            db.create_project(&format!("p{i}"), &format!("Project {i}"), "", None, "test")
                .unwrap();
        }

        let page = db.list_projects(None, None, Some(3), None).unwrap();
        assert_eq!(page.len(), 3);

        let page2 = db.list_projects(None, None, Some(3), Some(3)).unwrap();
        assert_eq!(page2.len(), 2);
    }

    #[test]
    fn create_project_with_parent() {
        let db = test_db();
        let parent = db
            .create_project("chitta", "Chitta", "", None, "test")
            .unwrap();
        let child = db
            .create_project(
                "chitta/research",
                "Chitta Research",
                "",
                Some(parent.id),
                "test",
            )
            .unwrap();
        assert_eq!(child.parent_id, Some(parent.id));
        assert_eq!(child.slug, "chitta/research");
    }

    #[test]
    fn list_projects_roots_only() {
        let db = test_db();
        let parent = db
            .create_project("chitta", "Chitta", "", None, "test")
            .unwrap();
        db.create_project("chitta/research", "Research", "", Some(parent.id), "test")
            .unwrap();
        db.create_project("yojana", "Yojana", "", None, "test")
            .unwrap();

        let roots = db.list_projects(None, Some(None), None, None).unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.iter().all(|p| p.parent_id.is_none()));
    }

    #[test]
    fn list_projects_children_of_parent() {
        let db = test_db();
        let parent = db
            .create_project("chitta", "Chitta", "", None, "test")
            .unwrap();
        db.create_project("chitta/core", "Core", "", Some(parent.id), "test")
            .unwrap();
        db.create_project("chitta/research", "Research", "", Some(parent.id), "test")
            .unwrap();
        db.create_project("yojana", "Yojana", "", None, "test")
            .unwrap();

        let children = db
            .list_projects(None, Some(Some(&parent.id)), None, None)
            .unwrap();
        assert_eq!(children.len(), 2);
        let slugs: Vec<&str> = children.iter().map(|p| p.slug.as_str()).collect();
        assert!(slugs.contains(&"chitta/core"));
        assert!(slugs.contains(&"chitta/research"));
    }

    #[test]
    fn descendant_project_ids() {
        let db = test_db();
        let root = db
            .create_project("chitta", "Chitta", "", None, "test")
            .unwrap();
        let child = db
            .create_project("chitta/research", "Research", "", Some(root.id), "test")
            .unwrap();
        db.create_project(
            "chitta/research/embed",
            "Embed Research",
            "",
            Some(child.id),
            "test",
        )
        .unwrap();
        db.create_project("chitta/core", "Core", "", Some(root.id), "test")
            .unwrap();

        let descendants = db.list_descendant_project_ids(&root.id).unwrap();
        assert_eq!(descendants.len(), 3);

        let all = db.project_ids_with_descendants(&root.id).unwrap();
        assert_eq!(all.len(), 4); // root + 3 descendants
    }

    #[test]
    fn task_query_rolls_up_descendants() {
        let db = test_db();
        let root = db
            .create_project("chitta", "Chitta", "", None, "test")
            .unwrap();
        let child = db
            .create_project("chitta/research", "Research", "", Some(root.id), "test")
            .unwrap();

        create_test_task(&db, "chitta", "Root task");
        // Create task in child project
        db.create_task(
            CreateTaskParams {
                project_id: child.id,
                project_slug: "chitta/research".into(),
                title: "Child task".into(),
                description: String::new(),
                category: None,
                status: None,
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
                arc_id: None,
                arc_phase: None,
            },
            "test",
        )
        .unwrap();

        let all_ids = db.project_ids_with_descendants(&root.id).unwrap();
        let tasks = db
            .list_tasks(&TaskQueryFilter {
                project_ids: Some(all_ids),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(tasks.len(), 2);

        // Query just the child
        let child_tasks = db
            .list_tasks(&TaskQueryFilter {
                project_ids: Some(vec![child.id]),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(child_tasks.len(), 1);
        assert_eq!(child_tasks[0].title, "Child task");
    }

    #[test]
    fn parse_task_id_with_nested_slug() {
        let db = test_db();
        let root = db
            .create_project("chitta", "Chitta", "", None, "test")
            .unwrap();
        let child = db
            .create_project("chitta/research", "Research", "", Some(root.id), "test")
            .unwrap();
        db.create_task(
            CreateTaskParams {
                project_id: child.id,
                project_slug: "chitta/research".into(),
                title: "Nested task".into(),
                description: String::new(),
                category: None,
                status: None,
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
                arc_id: None,
                arc_phase: None,
            },
            "test",
        )
        .unwrap();

        let task = db.get_task("chitta/research/1").unwrap().unwrap();
        assert_eq!(task.title, "Nested task");
        assert_eq!(task.project_slug, "chitta/research");
    }

    // --- Arc tests ---

    #[test]
    fn create_arc_and_get_by_slug() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();

        let phases = serde_json::to_string(&serde_json::json!([
            {"name": "design", "slice_type": "HITL", "gate": "manual"},
            {"name": "implement", "slice_type": "AFK", "gate": "auto"},
            {"name": "review", "slice_type": "AFK", "gate": "manual"}
        ]))
        .unwrap();

        let arc = db
            .create_arc(
                CreateArcParams {
                    project_id: p.id,
                    project_slug: p.slug.clone(),
                    title: "Feature X".into(),
                    description: "Build feature X".into(),
                    phases,
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();

        assert_eq!(arc.sequence_number, 1);
        assert_eq!(arc.title, "Feature X");
        assert_eq!(arc.status, "active");
        assert_eq!(arc.project_slug, "proj");

        // Phase defaults: first = active, rest = pending
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&arc.phases).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0]["status"], "active");
        assert_eq!(parsed[1]["status"], "pending");
        assert_eq!(parsed[2]["status"], "pending");

        // History records creation
        let history: Vec<HistoryEntry> = serde_json::from_str(&arc.history).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].kind, "arc_created");

        // Get by slug/~N
        let fetched = db.get_arc("proj/~1").unwrap().unwrap();
        assert_eq!(fetched.id, arc.id);
        assert_eq!(fetched.title, "Feature X");
    }

    #[test]
    fn arc_sequence_numbers_are_per_project() {
        let db = test_db();
        db.create_project("alpha", "Alpha", "", None, "test")
            .unwrap();
        db.create_project("beta", "Beta", "", None, "test").unwrap();
        let pa = db.get_project(None, Some("alpha")).unwrap().unwrap();
        let pb = db.get_project(None, Some("beta")).unwrap().unwrap();

        let phases = r#"[{"name":"do"}]"#.to_string();

        let a1 = db
            .create_arc(
                CreateArcParams {
                    project_id: pa.id,
                    project_slug: pa.slug.clone(),
                    title: "A1".into(),
                    description: String::new(),
                    phases: phases.clone(),
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();
        let b1 = db
            .create_arc(
                CreateArcParams {
                    project_id: pb.id,
                    project_slug: pb.slug.clone(),
                    title: "B1".into(),
                    description: String::new(),
                    phases: phases.clone(),
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();
        let a2 = db
            .create_arc(
                CreateArcParams {
                    project_id: pa.id,
                    project_slug: pa.slug.clone(),
                    title: "A2".into(),
                    description: String::new(),
                    phases,
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();

        assert_eq!(a1.sequence_number, 1);
        assert_eq!(b1.sequence_number, 1);
        assert_eq!(a2.sequence_number, 2);
    }

    #[test]
    fn update_arc_records_history() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();

        let arc = db
            .create_arc(
                CreateArcParams {
                    project_id: p.id,
                    project_slug: p.slug.clone(),
                    title: "Original".into(),
                    description: String::new(),
                    phases: r#"[{"name":"do"}]"#.into(),
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();

        let updated = db
            .update_arc(
                &format!("proj/~{}", arc.sequence_number),
                ArcUpdates {
                    title: Some("Renamed".into()),
                    status: Some("paused".into()),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();

        assert_eq!(updated.title, "Renamed");
        assert_eq!(updated.status, "paused");

        let history: Vec<HistoryEntry> = serde_json::from_str(&updated.history).unwrap();
        assert_eq!(history.len(), 3); // created + title updated + status changed
        assert_eq!(history[1].kind, "updated");
        assert_eq!(history[2].kind, "status_changed");
    }

    #[test]
    fn update_arc_rejects_invalid_status() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();

        db.create_arc(
            CreateArcParams {
                project_id: p.id,
                project_slug: p.slug,
                title: "Arc".into(),
                description: String::new(),
                phases: r#"[{"name":"do"}]"#.into(),
                tags: "[]".into(),
                context_refs: "[]".into(),
            },
            "test",
        )
        .unwrap();

        let err = db
            .update_arc(
                "proj/~1",
                ArcUpdates {
                    status: Some("bogus".into()),
                    ..Default::default()
                },
                "test",
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid arc status"));
    }

    #[test]
    fn task_with_arc_assignment() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();

        let arc = db
            .create_arc(
                CreateArcParams {
                    project_id: p.id,
                    project_slug: p.slug.clone(),
                    title: "Arc".into(),
                    description: String::new(),
                    phases: r#"[{"name":"design"},{"name":"implement"}]"#.into(),
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();

        let task = db
            .create_task(
                CreateTaskParams {
                    project_id: p.id,
                    project_slug: p.slug.clone(),
                    title: "Design doc".into(),
                    description: String::new(),
                    category: None,
                    status: None,
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
                    arc_id: Some(arc.id),
                    arc_phase: Some("design".into()),
                },
                "test",
            )
            .unwrap();

        assert_eq!(task.arc_id, Some(arc.id));
        assert_eq!(task.arc_phase.as_deref(), Some("design"));

        // Task without arc
        let plain = create_test_task(&db, "proj", "No arc");
        assert!(plain.arc_id.is_none());
        assert!(plain.arc_phase.is_none());
    }

    #[test]
    fn task_update_arc_assignment() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();

        let arc = db
            .create_arc(
                CreateArcParams {
                    project_id: p.id,
                    project_slug: p.slug.clone(),
                    title: "Arc".into(),
                    description: String::new(),
                    phases: r#"[{"name":"design"},{"name":"implement"}]"#.into(),
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();

        let task = create_test_task(&db, "proj", "Task");
        assert!(task.arc_id.is_none());

        let updated = db
            .update_task(
                &format!("proj/{}", task.sequence_number),
                TaskUpdates {
                    arc_id: Some(Some(arc.id)),
                    arc_phase: Some(Some("implement".into())),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();

        assert_eq!(updated.arc_id, Some(arc.id));
        assert_eq!(updated.arc_phase.as_deref(), Some("implement"));

        // Clear arc assignment
        let cleared = db
            .update_task(
                &format!("proj/{}", task.sequence_number),
                TaskUpdates {
                    arc_id: Some(None),
                    arc_phase: Some(None),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();

        assert!(cleared.arc_id.is_none());
        assert!(cleared.arc_phase.is_none());
    }

    #[test]
    fn query_tasks_by_arc() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();

        let arc = db
            .create_arc(
                CreateArcParams {
                    project_id: p.id,
                    project_slug: p.slug.clone(),
                    title: "Arc".into(),
                    description: String::new(),
                    phases: r#"[{"name":"design"},{"name":"implement"}]"#.into(),
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();

        // Create tasks: 2 in arc, 1 outside
        db.create_task(
            CreateTaskParams {
                project_id: p.id,
                project_slug: p.slug.clone(),
                title: "Design task".into(),
                description: String::new(),
                category: None,
                status: None,
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
                arc_id: Some(arc.id),
                arc_phase: Some("design".into()),
            },
            "test",
        )
        .unwrap();
        db.create_task(
            CreateTaskParams {
                project_id: p.id,
                project_slug: p.slug.clone(),
                title: "Impl task".into(),
                description: String::new(),
                category: None,
                status: None,
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
                arc_id: Some(arc.id),
                arc_phase: Some("implement".into()),
            },
            "test",
        )
        .unwrap();
        create_test_task(&db, "proj", "Unrelated");

        let arc_tasks = db
            .list_tasks(&TaskQueryFilter {
                arc_id: Some(arc.id),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(arc_tasks.len(), 2);

        let all_tasks = db.list_tasks(&TaskQueryFilter::default()).unwrap();
        assert_eq!(all_tasks.len(), 3);
    }

    // --- Auto-advance tests ---

    fn setup_arc_with_auto_gate(db: &Db) -> (Uuid, Uuid) {
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();
        let phases = serde_json::to_string(&serde_json::json!([
            {"name": "design", "gate": "auto"},
            {"name": "implement", "gate": "auto"},
        ]))
        .unwrap();
        let arc = db
            .create_arc(
                CreateArcParams {
                    project_id: p.id,
                    project_slug: "proj".into(),
                    title: "Test Arc".into(),
                    description: String::new(),
                    phases,
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();
        (p.id, arc.id)
    }

    fn create_arc_task(db: &Db, project_id: Uuid, arc_id: Uuid, phase: &str) -> TaskRow {
        db.create_task(
            CreateTaskParams {
                project_id,
                project_slug: "proj".into(),
                title: format!("Task in {phase}"),
                description: String::new(),
                category: None,
                status: None,
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
                arc_id: Some(arc_id),
                arc_phase: Some(phase.into()),
            },
            "test",
        )
        .unwrap()
    }

    #[test]
    fn auto_advance_completes_phase_and_activates_next() {
        let db = test_db();
        let (pid, arc_id) = setup_arc_with_auto_gate(&db);
        let t1 = create_arc_task(&db, pid, arc_id, "design");

        // Transition to done: needs-triage → in-progress → done
        advance_to(
            &db,
            &format!("proj/{}", t1.sequence_number),
            &["in-progress", "done"],
        );

        let result = db.try_auto_advance_phase(&t1.id, "test").unwrap();
        assert!(result, "should have auto-advanced");

        let arc = db.get_arc("proj/~1").unwrap().unwrap();
        let phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases).unwrap();
        assert_eq!(phases[0]["status"], "completed");
        assert_eq!(phases[1]["status"], "active");
    }

    #[test]
    fn auto_advance_wontfix_counts_as_terminal() {
        let db = test_db();
        let (pid, arc_id) = setup_arc_with_auto_gate(&db);
        let t1 = create_arc_task(&db, pid, arc_id, "design");

        advance_to(&db, &format!("proj/{}", t1.sequence_number), &["wontfix"]);

        let result = db.try_auto_advance_phase(&t1.id, "test").unwrap();
        assert!(result);

        let arc = db.get_arc("proj/~1").unwrap().unwrap();
        let phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases).unwrap();
        assert_eq!(phases[0]["status"], "completed");
    }

    #[test]
    fn auto_advance_skipped_when_not_all_terminal() {
        let db = test_db();
        let (pid, arc_id) = setup_arc_with_auto_gate(&db);
        let t1 = create_arc_task(&db, pid, arc_id, "design");
        let _t2 = create_arc_task(&db, pid, arc_id, "design");

        advance_to(
            &db,
            &format!("proj/{}", t1.sequence_number),
            &["in-progress", "done"],
        );

        let result = db.try_auto_advance_phase(&t1.id, "test").unwrap();
        assert!(!result, "should NOT advance — t2 still open");

        let arc = db.get_arc("proj/~1").unwrap().unwrap();
        let phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases).unwrap();
        assert_eq!(phases[0]["status"], "active");
    }

    #[test]
    fn auto_advance_skipped_for_manual_gate() {
        let db = test_db();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
        let p = db.get_project(None, Some("proj")).unwrap().unwrap();
        let phases = serde_json::to_string(&serde_json::json!([
            {"name": "design", "gate": "manual"},
            {"name": "implement", "gate": "auto"},
        ]))
        .unwrap();
        let arc = db
            .create_arc(
                CreateArcParams {
                    project_id: p.id,
                    project_slug: "proj".into(),
                    title: "Manual Arc".into(),
                    description: String::new(),
                    phases,
                    tags: "[]".into(),
                    context_refs: "[]".into(),
                },
                "test",
            )
            .unwrap();
        let t1 = create_arc_task(&db, p.id, arc.id, "design");
        advance_to(
            &db,
            &format!("proj/{}", t1.sequence_number),
            &["in-progress", "done"],
        );

        let result = db.try_auto_advance_phase(&t1.id, "test").unwrap();
        assert!(!result, "manual gate should not auto-advance");

        let arc = db.get_arc("proj/~1").unwrap().unwrap();
        let phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases).unwrap();
        assert_eq!(phases[0]["status"], "active");
    }

    #[test]
    fn auto_advance_skipped_for_zero_task_phase() {
        let db = test_db();
        let (_pid, _arc_id) = setup_arc_with_auto_gate(&db);

        // Create a fake task not in any arc to test the no-arc path
        let task = db
            .create_task(
                CreateTaskParams {
                    project_id: _pid,
                    project_slug: "proj".into(),
                    title: "Plain task".into(),
                    description: String::new(),
                    category: None,
                    status: None,
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
                    arc_id: None,
                    arc_phase: None,
                },
                "test",
            )
            .unwrap();

        let result = db.try_auto_advance_phase(&task.id, "test").unwrap();
        assert!(!result, "task without arc should return false");
    }

    #[test]
    fn auto_advance_history_is_distinguishable() {
        let db = test_db();
        let (pid, arc_id) = setup_arc_with_auto_gate(&db);
        let t1 = create_arc_task(&db, pid, arc_id, "design");
        advance_to(
            &db,
            &format!("proj/{}", t1.sequence_number),
            &["in-progress", "done"],
        );

        db.try_auto_advance_phase(&t1.id, "test").unwrap();

        let arc = db.get_arc("proj/~1").unwrap().unwrap();
        let history: Vec<HistoryEntry> = serde_json::from_str(&arc.history).unwrap();
        let auto_entry = history.iter().find(|e| e.kind == "phase_auto_advanced");
        assert!(
            auto_entry.is_some(),
            "should have phase_auto_advanced entry"
        );
        let entry = auto_entry.unwrap();
        assert_eq!(entry.actor, Some("test".into()));
        let payload = &entry.payload;
        assert_eq!(payload["phase"], "design");
        assert_eq!(payload["to"], "completed");
    }
}
