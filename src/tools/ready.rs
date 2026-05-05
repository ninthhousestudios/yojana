use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{Db, TaskQueryFilter, TaskRow};
use crate::error::YojanaError;
use crate::graph;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadyArgs {
    /// Project id or slug (optional — omit for cross-project ready check)
    #[serde(default)]
    pub project: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReadyItem {
    pub id: String,
    pub project_slug: String,
    pub human_id: String,
    pub title: String,
    pub status: String,
    pub category: Option<String>,
    pub slice_type: Option<String>,
    pub updated_at: i64,
}

impl From<TaskRow> for ReadyItem {
    fn from(t: TaskRow) -> Self {
        Self {
            id: t.id.to_string(),
            project_slug: t.project_slug.clone(),
            human_id: format!("{}/{}", t.project_slug, t.sequence_number),
            title: t.title,
            status: t.status,
            category: t.category,
            slice_type: t.slice_type,
            updated_at: t.updated_at,
        }
    }
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

pub fn handle(db: &Db, args: ReadyArgs) -> Result<serde_json::Value, YojanaError> {
    let project_ids = args
        .project
        .as_deref()
        .map(|p| resolve_project_ids(db, p))
        .transpose()?;

    let deps = db.list_depends_on_with_status()?;

    let mut ready_tasks = Vec::new();

    for status in &["ready-for-agent", "ready-for-human"] {
        let filter = TaskQueryFilter {
            project_ids: project_ids.clone(),
            status: Some((*status).to_string()),
            ..Default::default()
        };
        let tasks = db.list_tasks(&filter)?;
        for t in tasks {
            if graph::is_ready(t.id, &deps) {
                ready_tasks.push(ReadyItem::from(t));
            }
        }
    }

    Ok(serde_json::to_value(ready_tasks)?)
}
