use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use uuid::Uuid;

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
    /// Parent project slug or id (for list: filter children of this parent)
    #[serde(default)]
    pub parent: Option<String>,
    /// Handoff note (for update). Set to write; set to empty string to clear.
    #[serde(default)]
    pub handoff: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectOutput {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ProjectChild>>,
    pub history: Vec<HistoryEntry>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct ProjectChild {
    pub slug: String,
    pub title: String,
    pub status: String,
}

impl ProjectOutput {
    fn from(row: ProjectRow) -> Self {
        let history: Vec<HistoryEntry> = serde_json::from_str(&row.history).unwrap_or_default();
        Self {
            id: row.id.to_string(),
            slug: row.slug,
            title: row.title,
            description: row.description,
            status: row.status,
            parent_id: row.parent_id.map(|id| id.to_string()),
            handoff: row.handoff,
            children: None,
            history,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn with_children(row: ProjectRow, children: Vec<ProjectRow>) -> Self {
        let mut out = Self::from(row);
        out.children = Some(
            children
                .into_iter()
                .map(|c| ProjectChild {
                    slug: c.slug,
                    title: c.title,
                    status: c.status,
                })
                .collect(),
        );
        out
    }
}

fn validate_slug(slug: &str) -> Result<(), YojanaError> {
    if slug.is_empty() || slug.starts_with('/') || slug.ends_with('/') || slug.contains("//") {
        return Err(YojanaError::InvalidInput(
            "slug must be non-empty, no leading/trailing/double slashes".into(),
        ));
    }
    for segment in slug.split('/') {
        if segment.is_empty() {
            return Err(YojanaError::InvalidInput(
                "slug segments must be non-empty".into(),
            ));
        }
        if !segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(YojanaError::InvalidInput(
                "slug segments must be lowercase alphanumeric with hyphens".into(),
            ));
        }
        if segment.chars().all(|c| c.is_ascii_digit()) {
            return Err(YojanaError::InvalidInput(
                "slug segments must contain at least one non-digit character".into(),
            ));
        }
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

fn resolve_parent_id(db: &Db, slug: &str) -> Result<Option<Uuid>, YojanaError> {
    if let Some((parent_slug, _)) = slug.rsplit_once('/') {
        let parent = db
            .get_project(None, Some(parent_slug))?
            .ok_or_else(|| {
                YojanaError::InvalidInput(format!(
                    "parent project '{parent_slug}' does not exist; create it first"
                ))
            })?;
        Ok(Some(parent.id))
    } else {
        Ok(None)
    }
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
            let parent_id = resolve_parent_id(db, slug)?;
            let row = db.create_project(slug, title, description, parent_id)?;
            Ok(serde_json::to_value(ProjectOutput::from(row))?)
        }
        "get" => {
            let row = db
                .get_project(args.id.as_deref(), args.slug.as_deref())?
                .ok_or_else(|| YojanaError::NotFound("project not found".into()))?;
            let children = db.list_projects(None, Some(Some(&row.id)), None, None)?;
            let out = if children.is_empty() {
                ProjectOutput::from(row)
            } else {
                ProjectOutput::with_children(row, children)
            };
            Ok(serde_json::to_value(out)?)
        }
        "list" => {
            if let Some(ref status) = args.status {
                validate_status(status)?;
            }
            let parent_filter = if let Some(ref parent) = args.parent {
                let parent_row = if let Ok(uuid) = Uuid::parse_str(parent) {
                    db.get_project(Some(&uuid.to_string()), None)?
                } else {
                    db.get_project(None, Some(parent))?
                };
                let parent_row = parent_row.ok_or_else(|| {
                    YojanaError::NotFound(format!("parent project '{parent}' not found"))
                })?;
                Some(Some(parent_row.id))
            } else {
                Some(None) // roots only by default
            };
            let parent_ref = parent_filter.as_ref().map(|o| o.as_ref());
            let rows = db.list_projects(args.status.as_deref(), parent_ref, None, None)?;
            let out: Vec<ProjectOutput> = rows.into_iter().map(ProjectOutput::from).collect();
            Ok(serde_json::to_value(out)?)
        }
        "update" => {
            if let Some(ref status) = args.status {
                validate_status(status)?;
            }
            let handoff = args.handoff.map(|h| {
                if h.is_empty() { None } else { Some(h) }
            });
            let row = db.update_project(
                args.id.as_deref(),
                args.slug.as_deref(),
                ProjectUpdates {
                    title: args.title,
                    description: args.description,
                    status: args.status,
                    handoff,
                },
            )?;
            Ok(serde_json::to_value(ProjectOutput::from(row))?)
        }
        other => Err(YojanaError::InvalidInput(format!(
            "unknown action '{other}'; valid: create, get, list, update"
        ))),
    }
}
