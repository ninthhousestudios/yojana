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
    /// Max results to return (default 100)
    #[serde(default)]
    pub limit: Option<i64>,
    /// Offset for pagination
    #[serde(default)]
    pub offset: Option<i64>,
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
}

fn resolve_project_id(db: &Db, project: &str) -> Result<Uuid, YojanaError> {
    let row = if Uuid::parse_str(project).is_ok() {
        db.get_project(Some(project), None)?
    } else {
        db.get_project(None, Some(project))?
    };
    let row = row.ok_or_else(|| YojanaError::NotFound(format!("project '{project}'")))?;
    Ok(row.id)
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
            }
        })
        .collect()
}

fn is_ready_status(status: &str) -> bool {
    status == "ready-for-agent" || status == "ready-for-human"
}

pub fn handle(db: &Db, args: QueryArgs) -> Result<serde_json::Value, YojanaError> {
    let project_id = args
        .project
        .as_deref()
        .map(|p| resolve_project_id(db, p))
        .transpose()?;

    let filter = TaskQueryFilter {
        project_id,
        status: args.status,
        category: args.category,
        slice_type: args.slice_type,
        tag: args.tag,
        limit: args.limit,
        offset: args.offset,
    };

    let tasks = db.list_tasks(&filter)?;
    let deps = db.list_depends_on_with_status()?;
    let results = enrich(tasks, &deps);
    Ok(serde_json::to_value(results)?)
}
