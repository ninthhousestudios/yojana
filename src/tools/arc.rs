use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{ArcRow, CreateArcParams, Db, HistoryEntry, ArcUpdates};
use crate::error::YojanaError;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ArcArgs {
    /// Action: "create", "get", "update"
    pub action: String,
    /// Arc UUID or "project-slug/~N" (for get/update)
    #[serde(default)]
    pub id: Option<String>,
    /// Project id or slug (required for create)
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Arc status: "active", "paused", "completed", "abandoned"
    #[serde(default)]
    pub status: Option<String>,
    /// Phase definitions (required for create). Array of {name, slice_type?, gate?}
    #[serde(default)]
    pub phases: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub context_refs: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize)]
pub struct ArcOutput {
    pub id: String,
    pub project_id: String,
    pub project_slug: String,
    pub human_id: String,
    pub sequence_number: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub phases: Vec<serde_json::Value>,
    pub tags: Vec<String>,
    pub context_refs: Vec<serde_json::Value>,
    pub history: Vec<HistoryEntry>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<ArcRow> for ArcOutput {
    fn from(row: ArcRow) -> Self {
        let human_id = format!("{}/~{}", row.project_slug, row.sequence_number);
        Self {
            id: row.id.to_string(),
            project_id: row.project_id.to_string(),
            project_slug: row.project_slug,
            human_id,
            sequence_number: row.sequence_number,
            title: row.title,
            description: row.description,
            status: row.status,
            phases: serde_json::from_str(&row.phases).unwrap_or_default(),
            tags: serde_json::from_str(&row.tags).unwrap_or_default(),
            context_refs: serde_json::from_str(&row.context_refs).unwrap_or_default(),
            history: serde_json::from_str(&row.history).unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn to_json(val: &(impl serde::Serialize + ?Sized)) -> Result<String, YojanaError> {
    serde_json::to_string(val).map_err(YojanaError::Json)
}

fn resolve_project(db: &Db, project: &str) -> Result<(Uuid, String), YojanaError> {
    let row = if Uuid::parse_str(project).is_ok() {
        db.get_project(Some(project), None)?
    } else {
        db.get_project(None, Some(project))?
    };
    let row = row.ok_or_else(|| YojanaError::NotFound(format!("project '{project}'")))?;
    Ok((row.id, row.slug))
}

pub fn handle(db: &Db, args: ArcArgs) -> Result<serde_json::Value, YojanaError> {
    match args.action.as_str() {
        "create" => {
            let project = args
                .project
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("project required for create".into()))?;
            let title = args
                .title
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("title required for create".into()))?;
            let phases = args
                .phases
                .as_ref()
                .ok_or_else(|| YojanaError::InvalidInput("phases required for create".into()))?;

            let (project_id, project_slug) = resolve_project(db, project)?;

            let params = CreateArcParams {
                project_id,
                project_slug,
                title: title.to_string(),
                description: args.description.unwrap_or_default(),
                phases: to_json(phases)?,
                tags: to_json(&args.tags.unwrap_or_default())?,
                context_refs: to_json(&args.context_refs.unwrap_or_default())?,
            };
            let row = db.create_arc(params)?;
            Ok(serde_json::to_value(ArcOutput::from(row))?)
        }
        "get" => {
            let id = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for get".into()))?;
            let row = db
                .get_arc(id)?
                .ok_or_else(|| YojanaError::NotFound(format!("arc '{id}'")))?;
            Ok(serde_json::to_value(ArcOutput::from(row))?)
        }
        "update" => {
            let id = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for update".into()))?;

            let updates = ArcUpdates {
                title: args.title,
                description: args.description,
                status: args.status,
                tags: args.tags.map(|v| to_json(&v)).transpose()?,
                context_refs: args.context_refs.map(|v| to_json(&v)).transpose()?,
            };
            let row = db.update_arc(id, updates)?;
            Ok(serde_json::to_value(ArcOutput::from(row))?)
        }
        other => Err(YojanaError::InvalidInput(format!(
            "unknown action '{other}'; valid: create, get, update"
        ))),
    }
}
