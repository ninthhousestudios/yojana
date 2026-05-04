use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{CreateTaskParams, Db, HistoryEntry, TaskRow, TaskUpdates};
use crate::error::YojanaError;

const VALID_CATEGORIES: &[&str] = &["bug", "enhancement", "experiment"];
const VALID_REF_TYPES: &[&str] = &[
    "smriti:hash",
    "smriti:path",
    "sutra:symbol",
    "kosha:citation",
    "yojana:task",
    "chitta:memory",
    "doc:path",
    "git:commit",
    "git:range",
];
const VALID_SLICE_TYPES: &[&str] = &["AFK", "HITL"];

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskArgs {
    /// Action: "create", "get", "update", "comment"
    pub action: String,
    /// Task UUID or "project-slug/N" (for get/update/comment)
    #[serde(default)]
    pub id: Option<String>,
    /// Project id or slug (required for create)
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// "bug", "enhancement", or "experiment"
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    /// "AFK" or "HITL"
    #[serde(default)]
    pub slice_type: Option<String>,
    #[serde(default)]
    pub acceptance_criteria: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub decisions: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub context_refs: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub files: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub implementation_plan: Option<String>,
    #[serde(default)]
    pub execution_record: Option<String>,
    #[serde(default)]
    pub reproduction: Option<String>,
    #[serde(default)]
    pub root_cause: Option<String>,
    /// Comment text (for action=comment)
    #[serde(default)]
    pub text: Option<String>,
    /// Comment author (for action=comment, defaults to "user")
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskOutput {
    pub id: String,
    pub project_id: String,
    pub project_slug: String,
    pub human_id: String,
    pub sequence_number: i64,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub status: String,
    pub slice_type: Option<String>,
    pub acceptance_criteria: Vec<serde_json::Value>,
    pub decisions: Vec<serde_json::Value>,
    pub implementation_plan: Option<String>,
    pub execution_record: Option<String>,
    pub reproduction: Option<String>,
    pub root_cause: Option<String>,
    pub context_refs: Vec<serde_json::Value>,
    pub files: Vec<String>,
    pub tags: Vec<String>,
    pub history: Vec<HistoryEntry>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<TaskRow> for TaskOutput {
    fn from(row: TaskRow) -> Self {
        let human_id = format!("{}/{}", row.project_slug, row.sequence_number);
        Self {
            id: row.id.to_string(),
            project_id: row.project_id.to_string(),
            project_slug: row.project_slug,
            human_id,
            sequence_number: row.sequence_number,
            title: row.title,
            description: row.description,
            category: row.category,
            status: row.status,
            slice_type: row.slice_type,
            acceptance_criteria: json_array(&row.acceptance_criteria),
            decisions: json_array(&row.decisions),
            implementation_plan: row.implementation_plan,
            execution_record: row.execution_record,
            reproduction: row.reproduction,
            root_cause: row.root_cause,
            context_refs: json_array(&row.context_refs),
            files: serde_json::from_str(&row.files).unwrap_or_default(),
            tags: serde_json::from_str(&row.tags).unwrap_or_default(),
            history: serde_json::from_str(&row.history).unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn json_array(s: &str) -> Vec<serde_json::Value> {
    serde_json::from_str(s).unwrap_or_default()
}

fn validate_category(cat: &str) -> Result<(), YojanaError> {
    if !VALID_CATEGORIES.contains(&cat) {
        return Err(YojanaError::InvalidInput(format!(
            "invalid category '{cat}'; valid: {}",
            VALID_CATEGORIES.join(", ")
        )));
    }
    Ok(())
}

fn validate_slice_type(st: &str) -> Result<(), YojanaError> {
    if !VALID_SLICE_TYPES.contains(&st) {
        return Err(YojanaError::InvalidInput(format!(
            "invalid slice_type '{st}'; valid: {}",
            VALID_SLICE_TYPES.join(", ")
        )));
    }
    Ok(())
}

fn validate_context_refs(refs: &[serde_json::Value]) -> Result<(), YojanaError> {
    for r in refs {
        let ref_type = r
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                YojanaError::InvalidInput("context_ref must have a 'type' string".into())
            })?;
        if !VALID_REF_TYPES.contains(&ref_type) {
            return Err(YojanaError::InvalidInput(format!(
                "unknown context_ref type '{ref_type}'; valid: {}",
                VALID_REF_TYPES.join(", ")
            )));
        }
        r.get("value").and_then(|v| v.as_str()).ok_or_else(|| {
            YojanaError::InvalidInput("context_ref must have a 'value' string".into())
        })?;
    }
    Ok(())
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

pub fn handle(db: &Db, args: TaskArgs) -> Result<serde_json::Value, YojanaError> {
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
            if let Some(ref cat) = args.category {
                validate_category(cat)?;
            }
            if let Some(ref st) = args.slice_type {
                validate_slice_type(st)?;
            }
            let refs = args.context_refs.as_deref().unwrap_or(&[]);
            validate_context_refs(refs)?;

            let (project_id, project_slug) = resolve_project(db, project)?;

            let params = CreateTaskParams {
                project_id,
                project_slug,
                title: title.to_string(),
                description: args.description.unwrap_or_default(),
                category: args.category,
                slice_type: args.slice_type,
                acceptance_criteria: to_json(&args.acceptance_criteria.unwrap_or_default())?,
                decisions: to_json(&args.decisions.unwrap_or_default())?,
                context_refs: to_json(refs)?,
                files: to_json(&args.files.unwrap_or_default())?,
                tags: to_json(&args.tags.unwrap_or_default())?,
                implementation_plan: args.implementation_plan,
                execution_record: args.execution_record,
                reproduction: args.reproduction,
                root_cause: args.root_cause,
            };
            let row = db.create_task(params)?;
            Ok(serde_json::to_value(TaskOutput::from(row))?)
        }
        "get" => {
            let id = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for get".into()))?;
            let row = db
                .get_task(id)?
                .ok_or_else(|| YojanaError::NotFound(format!("task '{id}'")))?;
            Ok(serde_json::to_value(TaskOutput::from(row))?)
        }
        "update" => {
            let id = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for update".into()))?;
            if let Some(ref cat) = args.category {
                validate_category(cat)?;
            }
            if let Some(ref st) = args.slice_type {
                validate_slice_type(st)?;
            }
            if let Some(ref refs) = args.context_refs {
                validate_context_refs(refs)?;
            }
            let updates = TaskUpdates {
                title: args.title,
                description: args.description,
                category: args.category,
                status: args.status,
                slice_type: args.slice_type,
                acceptance_criteria: args
                    .acceptance_criteria
                    .map(|v| to_json(&v))
                    .transpose()?,
                decisions: args.decisions.map(|v| to_json(&v)).transpose()?,
                implementation_plan: args.implementation_plan,
                execution_record: args.execution_record,
                reproduction: args.reproduction,
                root_cause: args.root_cause,
                context_refs: args.context_refs.map(|v| to_json(&v)).transpose()?,
                files: args.files.map(|v| to_json(&v)).transpose()?,
                tags: args.tags.map(|v| to_json(&v)).transpose()?,
            };
            let row = db.update_task(id, updates)?;
            Ok(serde_json::to_value(TaskOutput::from(row))?)
        }
        "comment" => {
            let id = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for comment".into()))?;
            let text = args
                .text
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("text required for comment".into()))?;
            let task = db
                .get_task(id)?
                .ok_or_else(|| YojanaError::NotFound(format!("task '{id}'")))?;
            let message =
                db.append_conversation_message(&task.id, text, args.author.as_deref())?;
            Ok(serde_json::json!({
                "task": format!("{}/{}", task.project_slug, task.sequence_number),
                "message": message,
            }))
        }
        other => Err(YojanaError::InvalidInput(format!(
            "unknown action '{other}'; valid: create, get, update, comment"
        ))),
    }
}
