use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{Db, EdgeRow};
use crate::error::YojanaError;
use crate::tools::{reject_inapplicable_fields, supplied_fields};

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EdgeAction {
    Create,
    Delete,
    List,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    DependsOn,
    RelatesTo,
    Supersedes,
    Refines,
    MotivatedBy,
}

impl AsRef<str> for EdgeType {
    fn as_ref(&self) -> &str {
        match self {
            Self::DependsOn => "depends_on",
            Self::RelatesTo => "relates_to",
            Self::Supersedes => "supersedes",
            Self::Refines => "refines",
            Self::MotivatedBy => "motivated_by",
        }
    }
}

// Field docs omitted to keep the schema small (it reloads on summarization);
// semantics live in the tool-level description in src/mcp.rs.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EdgeArgs {
    pub action: EdgeAction,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub edge_type: Option<EdgeType>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub task: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EdgeOutput {
    pub id: String,
    pub source_task_id: String,
    pub target_task_id: String,
    pub edge_type: String,
    pub note: Option<String>,
    pub created_at: i64,
}

impl From<EdgeRow> for EdgeOutput {
    fn from(row: EdgeRow) -> Self {
        Self {
            id: row.id.to_string(),
            source_task_id: row.source_task_id.to_string(),
            target_task_id: row.target_task_id.to_string(),
            edge_type: row.edge_type,
            note: row.note,
            created_at: row.created_at,
        }
    }
}

fn resolve_task_id(db: &Db, identifier: &str) -> Result<Uuid, YojanaError> {
    let task = db
        .get_task(identifier)?
        .ok_or_else(|| YojanaError::NotFound(format!("task '{identifier}'")))?;
    Ok(task.id)
}

pub fn handle(db: &Db, args: EdgeArgs) -> Result<serde_json::Value, YojanaError> {
    let supplied = supplied_fields!(args; id, source, target, edge_type, note, task);
    let action = match args.action {
        EdgeAction::Create => "create",
        EdgeAction::Delete => "delete",
        EdgeAction::List => "list",
    };
    reject_inapplicable_fields(
        action,
        &supplied,
        |field| match args.action {
            EdgeAction::Create => matches!(field, "source" | "target" | "edge_type" | "note"),
            EdgeAction::Delete => field == "id",
            EdgeAction::List => field == "task",
        },
        &[],
    )?;
    match args.action {
        EdgeAction::Create => {
            let source = args
                .source
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("source required for create".into()))?;
            let target = args
                .target
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("target required for create".into()))?;
            let edge_type = args
                .edge_type
                .ok_or_else(|| YojanaError::InvalidInput("edge_type required for create".into()))?;

            let source_id = resolve_task_id(db, source)?;
            let target_id = resolve_task_id(db, target)?;

            let row = db.create_edge(
                source_id,
                target_id,
                edge_type.as_ref(),
                args.note.as_deref(),
            )?;
            Ok(serde_json::to_value(EdgeOutput::from(row))?)
        }
        EdgeAction::Delete => {
            let id_str = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for delete".into()))?;
            let id = Uuid::parse_str(id_str)
                .map_err(|_| YojanaError::InvalidInput(format!("invalid UUID '{id_str}'")))?;
            db.delete_edge(&id)?;
            Ok(serde_json::json!({"deleted": id_str}))
        }
        EdgeAction::List => {
            let task_ident = args
                .task
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("task required for list".into()))?;
            let task_id = resolve_task_id(db, task_ident)?;
            let rows = db.list_edges_for_task(&task_id)?;
            let out: Vec<EdgeOutput> = rows.into_iter().map(EdgeOutput::from).collect();
            Ok(serde_json::to_value(out)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(action: EdgeAction) -> EdgeArgs {
        EdgeArgs {
            action,
            id: None,
            source: None,
            target: None,
            edge_type: None,
            note: None,
            task: None,
        }
    }

    fn rejected_field(args: EdgeArgs, field: &str) {
        let db = Db::open_in_memory().expect("invariant: in-memory db opens");
        match handle(&db, args) {
            Err(YojanaError::InvalidInput(msg)) => assert!(msg.contains(field), "{msg}"),
            other => panic!("expected InvalidInput naming {field}, got {other:?}"),
        }
    }

    #[test]
    fn each_action_rejects_inapplicable_fields() {
        rejected_field(
            EdgeArgs {
                source: Some("proj/1".into()),
                target: Some("proj/2".into()),
                edge_type: Some(EdgeType::DependsOn),
                task: Some("proj/1".into()),
                ..blank(EdgeAction::Create)
            },
            "task",
        );
        rejected_field(
            EdgeArgs {
                id: Some(Uuid::nil().to_string()),
                note: Some("why".into()),
                ..blank(EdgeAction::Delete)
            },
            "note",
        );
        rejected_field(
            EdgeArgs {
                task: Some("proj/1".into()),
                edge_type: Some(EdgeType::DependsOn),
                ..blank(EdgeAction::List)
            },
            "edge_type",
        );
    }
}
