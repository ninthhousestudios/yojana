use std::collections::{BTreeMap, HashMap};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{Db, TaskQueryFilter, TaskRow};
use crate::error::YojanaError;
use crate::graph;

// Field docs omitted to keep the schema small (it reloads on summarization);
// semantics live in the tool-level description in src/mcp.rs.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryArgs {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub slice_type: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub arc: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default)]
    pub include_all_terminal: bool,
    #[serde(default)]
    pub recent_terminal_window_ms: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct QueryResultItem {
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

fn enrich(
    tasks: Vec<TaskRow>,
    deps_with_status: &[(Uuid, Uuid, String)],
    blocker_human_ids: &HashMap<Uuid, String>,
) -> Vec<QueryResultItem> {
    tasks
        .into_iter()
        .map(|t| {
            let ready = is_ready_status(&t.status) && graph::is_ready(t.id, deps_with_status);
            let blockers = graph::blocked_by(t.id, deps_with_status);
            let blocked = !blockers.is_empty();
            QueryResultItem {
                human_id: format!("{}/{}", t.project_slug, t.sequence_number),
                title: t.title,
                status: t.status,
                category: t.category,
                slice_type: t.slice_type,
                ready,
                blocked,
                blocked_by: blockers
                    .iter()
                    .map(|id| {
                        blocker_human_ids
                            .get(id)
                            .cloned()
                            .unwrap_or_else(|| id.to_string())
                    })
                    .collect(),
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
        .map(|a| db.resolve_arc_id(a).map(|(id, _)| id))
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
    // Resolve blocker UUIDs to human ids in one batch so rows carry actionable
    // "{slug}/{seq}" references instead of opaque UUIDs.
    let blocker_ids: Vec<Uuid> = tasks
        .iter()
        .flat_map(|t| graph::blocked_by(t.id, &deps))
        .collect();
    let blocker_human_ids = db.task_human_ids(&blocker_ids)?;
    let results = enrich(tasks, &deps, &blocker_human_ids);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{edge, task};

    fn test_db() -> Db {
        let db = Db::open_in_memory().unwrap();
        db.create_project("proj", "Project", "", None).unwrap();
        db
    }

    fn create_task(db: &Db, title: &str) -> String {
        let args = task::TaskArgs {
            action: "create".into(),
            id: None,
            project: Some("proj".into()),
            title: Some(title.into()),
            description: None,
            category: None,
            status: Some("ready-for-agent".into()),
            slice_type: None,
            acceptance_criteria: None,
            decisions: None,
            context_refs: None,
            files: None,
            tags: None,
            implementation_plan: None,
            execution_record: None,
            reproduction: None,
            root_cause: None,
            text: None,
            author: None,
            commit: None,
            arc_id: None,
            arc_phase: None,
        };
        let out = task::handle(db, args).unwrap();
        out["human_id"].as_str().unwrap().to_string()
    }

    fn query_all(db: &Db) -> Vec<serde_json::Value> {
        let out = handle(
            db,
            QueryArgs {
                project: Some("proj".into()),
                status: None,
                category: None,
                slice_type: None,
                tag: None,
                arc: None,
                limit: None,
                offset: None,
                include_all_terminal: false,
                recent_terminal_window_ms: None,
            },
        )
        .unwrap();
        out.as_array().unwrap().clone()
    }

    #[test]
    fn rows_drop_uuid_and_project_slug_keep_human_id() {
        let db = test_db();
        create_task(&db, "Solo");
        let rows = query_all(&db);
        let row = &rows[0];
        assert!(row.get("id").is_none(), "raw UUID should be gone");
        assert!(row.get("project_slug").is_none(), "project_slug is redundant");
        assert_eq!(row["human_id"], "proj/1");
    }

    #[test]
    fn blocked_by_uses_human_ids() {
        let db = test_db();
        let blocker = create_task(&db, "Blocker"); // proj/1
        let blocked = create_task(&db, "Blocked"); // proj/2
        edge::handle(
            &db,
            edge::EdgeArgs {
                action: "create".into(),
                id: None,
                source: Some(blocked.clone()),
                target: Some(blocker.clone()),
                edge_type: Some("depends_on".into()),
                note: None,
                task: None,
            },
        )
        .unwrap();

        let rows = query_all(&db);
        let blocked_row = rows
            .iter()
            .find(|r| r["human_id"] == "proj/2")
            .expect("blocked task present");
        let blocked_by: Vec<String> = blocked_row["blocked_by"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(blocked_by, vec![blocker], "blocker rendered as human_id, not UUID");
        assert_eq!(blocked_row["blocked"], true);
    }
}
