use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{Db, EdgeRow};
use crate::error::YojanaError;

// Field docs omitted to keep the schema small (it reloads on summarization);
// semantics live in the tool-level description in src/mcp.rs.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EdgeArgs {
    pub action: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub edge_type: Option<String>,
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
    match args.action.as_str() {
        "create" => {
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
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("edge_type required for create".into()))?;

            let source_id = resolve_task_id(db, source)?;
            let target_id = resolve_task_id(db, target)?;

            let row = db.create_edge(source_id, target_id, edge_type, args.note.as_deref())?;
            Ok(serde_json::to_value(EdgeOutput::from(row))?)
        }
        "delete" => {
            let id_str = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for delete".into()))?;
            let id = Uuid::parse_str(id_str)
                .map_err(|_| YojanaError::InvalidInput(format!("invalid UUID '{id_str}'")))?;
            db.delete_edge(&id)?;
            Ok(serde_json::json!({"deleted": id_str}))
        }
        "list" => {
            let task_ident = args
                .task
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("task required for list".into()))?;
            let task_id = resolve_task_id(db, task_ident)?;
            let rows = db.list_edges_for_task(&task_id)?;
            let out: Vec<EdgeOutput> = rows.into_iter().map(EdgeOutput::from).collect();
            Ok(serde_json::to_value(out)?)
        }
        other => Err(YojanaError::InvalidInput(format!(
            "unknown action '{other}'; valid: create, delete, list"
        ))),
    }
}
