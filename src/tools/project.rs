use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::db::{Db, HistoryEntry, ProjectRow, ProjectUpdates};
use crate::error::YojanaError;

const VALID_STATUSES: &[&str] = &["active", "paused", "archived"];

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectArgs {
    /// Action: "create", "get", "list", "update"
    pub action: String,
    /// Project UUID (for get/update)
    #[serde(default)]
    pub id: Option<String>,
    /// Project slug (for create, or get/update as alternative to id)
    #[serde(default)]
    pub slug: Option<String>,
    /// Project title (for create/update)
    #[serde(default)]
    pub title: Option<String>,
    /// Project description (for create/update)
    #[serde(default)]
    pub description: Option<String>,
    /// Project status filter (for list) or new status (for update): "active", "paused", "archived"
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectOutput {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub history: Vec<HistoryEntry>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<ProjectRow> for ProjectOutput {
    fn from(row: ProjectRow) -> Self {
        let history: Vec<HistoryEntry> = serde_json::from_str(&row.history).unwrap_or_default();
        Self {
            id: row.id.to_string(),
            slug: row.slug,
            title: row.title,
            description: row.description,
            status: row.status,
            history,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn validate_slug(slug: &str) -> Result<(), YojanaError> {
    if slug.is_empty()
        || !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(YojanaError::InvalidInput(
            "slug must be non-empty, lowercase alphanumeric with hyphens".into(),
        ));
    }
    Ok(())
}

fn validate_status(status: &str) -> Result<(), YojanaError> {
    if !VALID_STATUSES.contains(&status) {
        return Err(YojanaError::InvalidInput(format!(
            "invalid status '{status}'; valid: {}",
            VALID_STATUSES.join(", ")
        )));
    }
    Ok(())
}

pub fn handle(db: &Db, args: ProjectArgs) -> Result<serde_json::Value, YojanaError> {
    match args.action.as_str() {
        "create" => {
            let slug = args
                .slug
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("slug required for create".into()))?;
            validate_slug(slug)?;
            let title = args
                .title
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("title required for create".into()))?;
            let description = args.description.as_deref().unwrap_or("");
            let row = db.create_project(slug, title, description)?;
            Ok(serde_json::to_value(ProjectOutput::from(row))?)
        }
        "get" => {
            let row = db
                .get_project(args.id.as_deref(), args.slug.as_deref())?
                .ok_or_else(|| YojanaError::NotFound("project not found".into()))?;
            Ok(serde_json::to_value(ProjectOutput::from(row))?)
        }
        "list" => {
            if let Some(ref status) = args.status {
                validate_status(status)?;
            }
            let rows = db.list_projects(args.status.as_deref(), None, None)?;
            let out: Vec<ProjectOutput> = rows.into_iter().map(ProjectOutput::from).collect();
            Ok(serde_json::to_value(out)?)
        }
        "update" => {
            if let Some(ref status) = args.status {
                validate_status(status)?;
            }
            let row = db.update_project(
                args.id.as_deref(),
                args.slug.as_deref(),
                ProjectUpdates {
                    title: args.title,
                    description: args.description,
                    status: args.status,
                },
            )?;
            Ok(serde_json::to_value(ProjectOutput::from(row))?)
        }
        other => Err(YojanaError::InvalidInput(format!(
            "unknown action '{other}'; valid: create, get, list, update"
        ))),
    }
}
