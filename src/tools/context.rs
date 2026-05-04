use schemars::JsonSchema;
use serde::Deserialize;

use crate::context::{self, VALID_SHAPES};
use crate::db::Db;
use crate::error::YojanaError;

const DEFAULT_MAX_MESSAGES: usize = 10;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextArgs {
    /// Task id — UUID or "project-slug/N"
    pub task: String,
    /// Context shape: "summary" or "working"
    pub shape: String,
}

pub fn handle(db: &Db, args: ContextArgs) -> Result<serde_json::Value, YojanaError> {
    if !VALID_SHAPES.contains(&args.shape.as_str()) {
        return Err(YojanaError::InvalidInput(format!(
            "unknown shape '{}'; valid: {}",
            args.shape,
            VALID_SHAPES.join(", ")
        )));
    }

    let task = db
        .get_task(&args.task)?
        .ok_or_else(|| YojanaError::NotFound(format!("task '{}'", args.task)))?;
    let edges = db.list_edges_for_task(&task.id)?;

    match args.shape.as_str() {
        "summary" => {
            let bundle = context::summary(&task, &edges);
            Ok(serde_json::to_value(bundle)?)
        }
        "working" => {
            let nids = context::neighbor_ids(task.id, &edges);
            let mut neighbors_with_edges = Vec::new();
            for nid in &nids {
                if let Some(ntask) = db.get_task(&nid.to_string())? {
                    let nedges = db.list_edges_for_task(&ntask.id)?;
                    neighbors_with_edges.push((ntask, nedges));
                }
            }

            let messages = db.get_conversation_messages(&task.id)?;

            let bundle = context::working(
                &task,
                &neighbors_with_edges,
                &messages,
                DEFAULT_MAX_MESSAGES,
            );
            Ok(serde_json::to_value(bundle)?)
        }
        _ => unreachable!(),
    }
}
