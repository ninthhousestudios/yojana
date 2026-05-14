use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{Db, TaskQueryFilter, TaskRow};
use crate::error::YojanaError;
use crate::graph;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryArgs {
    /// Project id or slug (optional — omit for cross-project query)
    #[serde(default)]
    pub project: Option<String>,
    /// Filter by status
    #[serde(default)]
    pub status: Option<String>,
    /// Filter by category
    #[serde(default)]
    pub category: Option<String>,
    /// Filter by slice_type
    #[serde(default)]
    pub slice_type: Option<String>,
    /// Filter by tag (tasks containing this tag)
    #[serde(default)]
    pub tag: Option<String>,
    /// Filter by arc (UUID or "project-slug/~N"). When set, results are grouped by phase.
    #[serde(default)]
    pub arc: Option<String>,
    /// Max results to return (default 100)
    #[serde(default)]
    pub limit: Option<i64>,
    /// Offset for pagination
    #[serde(default)]
    pub offset: Option<i64>,
    /// If true, include all done/wontfix tasks. Default behavior includes them
    /// only if completed within `recent_terminal_window_ms` (default: last 24h).
    #[serde(default)]
    pub include_all_terminal: bool,
    /// Window in millis for "recent" done/wontfix tasks. Defaults to 24h.
    /// Ignored if `include_all_terminal` is true or `status` is set.
    #[serde(default)]
    pub recent_terminal_window_ms: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct QueryResultItem {
    pub id: String,
    pub project_slug: String,
    pub human_id: String,
    pub title: String,
    pub status: String,
    pub category: Option<String>,
    pub slice_type: Option<String>,
    pub ready: bool,
    pub blocked: bool,
    pub blocked_by: Vec<String>,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc_phase: Option<String>,
}

fn resolve_project_ids(db: &Db, project: &str) -> Result<Vec<Uuid>, YojanaError> {
    let row = if Uuid::parse_str(project).is_ok() {
        db.get_project(Some(project), None)?
    } else {
        db.get_project(None, Some(project))?
    };
    let row = row.ok_or_else(|| YojanaError::NotFound(format!("project '{project}'")))?;
    db.project_ids_with_descendants(&row.id)
}

fn enrich(tasks: Vec<TaskRow>, deps_with_status: &[(Uuid, Uuid, String)]) -> Vec<QueryResultItem> {
    tasks
        .into_iter()
        .map(|t| {
            let ready = is_ready_status(&t.status) && graph::is_ready(t.id, deps_with_status);
            let blockers = graph::blocked_by(t.id, deps_with_status);
            let blocked = !blockers.is_empty();
            QueryResultItem {
                id: t.id.to_string(),
                project_slug: t.project_slug.clone(),
                human_id: format!("{}/{}", t.project_slug, t.sequence_number),
                title: t.title,
                status: t.status,
                category: t.category,
                slice_type: t.slice_type,
                ready,
                blocked,
                blocked_by: blockers.iter().map(|id| id.to_string()).collect(),
                updated_at: t.updated_at,
                arc_phase: t.arc_phase,
            }
        })
        .collect()
}

fn is_ready_status(status: &str) -> bool {
    status == "ready-for-agent" || status == "ready-for-human"
}

pub fn handle(db: &Db, args: QueryArgs) -> Result<serde_json::Value, YojanaError> {
    let project_ids = args
        .project
        .as_deref()
        .map(|p| resolve_project_ids(db, p))
        .transpose()?;

    let arc_id = args
        .arc
        .as_deref()
        .map(|a| db.resolve_arc_id(a))
        .transpose()?;

    let cutoff = if args.include_all_terminal || args.status.is_some() {
        None
    } else {
        let window = args
            .recent_terminal_window_ms
            .unwrap_or(24 * 60 * 60 * 1000);
        Some(chrono::Utc::now().timestamp_millis() - window)
    };

    let filter = TaskQueryFilter {
        project_ids,
        status: args.status,
        category: args.category,
        slice_type: args.slice_type,
        tag: args.tag,
        limit: args.limit,
        offset: args.offset,
        include_terminal_after: cutoff,
        arc_id,
    };

    let tasks = db.list_tasks(&filter)?;
    let deps = db.list_depends_on_with_status()?;
    let results = enrich(tasks, &deps);

    if args.arc.is_some() {
        let mut grouped: BTreeMap<String, Vec<&QueryResultItem>> = BTreeMap::new();
        for item in &results {
            let phase = item
                .arc_phase
                .as_deref()
                .unwrap_or("unassigned")
                .to_string();
            grouped.entry(phase).or_default().push(item);
        }
        Ok(serde_json::to_value(grouped)?)
    } else {
        Ok(serde_json::to_value(results)?)
    }
}
