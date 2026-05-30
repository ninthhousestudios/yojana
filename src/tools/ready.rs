use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::collections::HashMap;

use crate::db::{ArcRow, Db, TaskQueryFilter, TaskRow};
use crate::error::YojanaError;
use crate::graph;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadyArgs {
    #[serde(default)]
    pub project: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HandoffEntry {
    pub project_slug: String,
    pub handoff: String,
}

#[derive(Debug, Serialize)]
pub struct ReadyItem {
    pub human_id: String,
    pub title: String,
    pub status: String,
    pub category: Option<String>,
    pub slice_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_slice_type: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct ReadyResponse {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub handoffs: Vec<HandoffEntry>,
    pub ready: Vec<ReadyItem>,
}

impl ReadyItem {
    fn from_task(t: TaskRow, phase_slice_type: Option<String>) -> Self {
        Self {
            human_id: format!("{}/{}", t.project_slug, t.sequence_number),
            title: t.title,
            status: t.status,
            category: t.category,
            slice_type: t.slice_type,
            phase_slice_type,
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

    let handoffs = if let Some(ref ids) = project_ids {
        db.get_handoffs(ids)?
    } else {
        db.get_handoffs(&all_active_project_ids(db)?)?
    };
    let handoff_entries: Vec<HandoffEntry> = handoffs
        .into_iter()
        .map(|(slug, text)| HandoffEntry {
            project_slug: slug,
            handoff: text,
        })
        .collect();

    let deps = db.list_depends_on_with_status()?;

    let mut arc_cache: HashMap<Uuid, ArcRow> = HashMap::new();
    let mut ready_tasks = Vec::new();

    for status in &["ready-for-agent", "ready-for-human"] {
        let filter = TaskQueryFilter {
            project_ids: project_ids.clone(),
            status: Some((*status).to_string()),
            ..Default::default()
        };
        let tasks = db.list_tasks(&filter)?;
        for t in tasks {
            if !graph::is_ready(t.id, &deps) {
                continue;
            }
            if let (Some(arc_id), Some(arc_phase)) = (t.arc_id, &t.arc_phase) {
                let arc = match arc_cache.entry(arc_id) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        if let Some(row) = db.get_arc(&arc_id.to_string())? {
                            e.insert(row)
                        } else {
                            continue;
                        }
                    }
                };
                if arc.status != "active" {
                    continue;
                }
                let phases: Vec<serde_json::Value> =
                    serde_json::from_str(&arc.phases).unwrap_or_default();
                let phase_obj = phases
                    .iter()
                    .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(arc_phase));
                let Some(phase_obj) = phase_obj else {
                    continue;
                };
                if phase_obj.get("status").and_then(|s| s.as_str()) != Some("active") {
                    continue;
                }
                let phase_st = phase_obj
                    .get("slice_type")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string());
                ready_tasks.push(ReadyItem::from_task(t, phase_st));
            } else {
                ready_tasks.push(ReadyItem::from_task(t, None));
            }
        }
    }

    Ok(serde_json::to_value(ReadyResponse {
        handoffs: handoff_entries,
        ready: ready_tasks,
    })?)
}

fn all_active_project_ids(db: &Db) -> Result<Vec<Uuid>, YojanaError> {
    let projects = db.list_projects(Some("active"), None, None, None)?;
    Ok(projects.into_iter().map(|p| p.id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{arc, task};

    fn test_db() -> Db {
        let db = Db::open_in_memory().unwrap();
        db.create_project("proj", "Project", "", None).unwrap();
        db
    }

    fn create_arc(db: &Db, phases: Vec<serde_json::Value>) -> serde_json::Value {
        arc::handle(
            db,
            arc::ArcArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some("Test Arc".into()),
                description: None,
                status: None,
                phases: Some(phases),
                tags: None,
                context_refs: None,
                phase: None,
                note: None,
                skip: None,
            },
        )
        .unwrap()
    }

    fn create_task_in_arc(db: &Db, title: &str, arc_id: &str, phase: &str) -> serde_json::Value {
        task::handle(
            db,
            task::TaskArgs {
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
                arc_id: Some(arc_id.into()),
                arc_phase: Some(phase.into()),
            },
        )
        .unwrap()
    }

    fn create_standalone_task(db: &Db, title: &str) -> serde_json::Value {
        task::handle(
            db,
            task::TaskArgs {
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
            },
        )
        .unwrap()
    }

    fn ready_titles(db: &Db) -> Vec<String> {
        let out = handle(
            db,
            ReadyArgs {
                project: Some("proj".into()),
            },
        )
        .unwrap();
        out["ready"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["title"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn active_arc_active_phase_appears_ready() {
        let db = test_db();
        create_arc(
            &db,
            vec![
                serde_json::json!({"name": "design", "slice_type": "HITL"}),
                serde_json::json!({"name": "implement", "slice_type": "AFK"}),
            ],
        );
        create_task_in_arc(&db, "Design doc", "proj/~1", "design");

        let titles = ready_titles(&db);
        assert!(titles.contains(&"Design doc".to_string()));
    }

    #[test]
    fn pending_phase_not_ready() {
        let db = test_db();
        create_arc(
            &db,
            vec![
                serde_json::json!({"name": "design"}),
                serde_json::json!({"name": "implement"}),
            ],
        );
        create_task_in_arc(&db, "Impl task", "proj/~1", "implement");

        let titles = ready_titles(&db);
        assert!(!titles.contains(&"Impl task".to_string()));
    }

    #[test]
    fn completed_phase_not_ready() {
        let db = test_db();
        create_arc(
            &db,
            vec![
                serde_json::json!({"name": "design"}),
                serde_json::json!({"name": "implement"}),
            ],
        );
        create_task_in_arc(&db, "Design doc", "proj/~1", "design");
        arc::handle(
            &db,
            arc::ArcArgs {
                action: "advance".into(),
                id: Some("proj/~1".into()),
                project: None,
                title: None,
                description: None,
                status: None,
                phases: None,
                tags: None,
                context_refs: None,
                phase: None,
                note: None,
                skip: None,
            },
        )
        .unwrap();

        let titles = ready_titles(&db);
        assert!(!titles.contains(&"Design doc".to_string()));
    }

    #[test]
    fn paused_arc_not_ready() {
        let db = test_db();
        create_arc(&db, vec![serde_json::json!({"name": "design"})]);
        create_task_in_arc(&db, "Design doc", "proj/~1", "design");
        arc::handle(
            &db,
            arc::ArcArgs {
                action: "update".into(),
                id: Some("proj/~1".into()),
                project: None,
                title: None,
                description: None,
                status: Some("paused".into()),
                phases: None,
                tags: None,
                context_refs: None,
                phase: None,
                note: None,
                skip: None,
            },
        )
        .unwrap();

        let titles = ready_titles(&db);
        assert!(!titles.contains(&"Design doc".to_string()));
    }

    #[test]
    fn abandoned_arc_not_ready() {
        let db = test_db();
        create_arc(&db, vec![serde_json::json!({"name": "design"})]);
        create_task_in_arc(&db, "Design doc", "proj/~1", "design");
        arc::handle(
            &db,
            arc::ArcArgs {
                action: "update".into(),
                id: Some("proj/~1".into()),
                project: None,
                title: None,
                description: None,
                status: Some("abandoned".into()),
                phases: None,
                tags: None,
                context_refs: None,
                phase: None,
                note: None,
                skip: None,
            },
        )
        .unwrap();

        let titles = ready_titles(&db);
        assert!(!titles.contains(&"Design doc".to_string()));
    }

    #[test]
    fn standalone_task_unaffected() {
        let db = test_db();
        create_standalone_task(&db, "Solo task");

        let titles = ready_titles(&db);
        assert!(titles.contains(&"Solo task".to_string()));
    }

    #[test]
    fn phase_slice_type_in_response() {
        let db = test_db();
        create_arc(
            &db,
            vec![serde_json::json!({"name": "design", "slice_type": "HITL"})],
        );
        create_task_in_arc(&db, "Design doc", "proj/~1", "design");

        let out = handle(
            &db,
            ReadyArgs {
                project: Some("proj".into()),
            },
        )
        .unwrap();
        let items = out["ready"].as_array().unwrap();
        let item = items.iter().find(|r| r["title"] == "Design doc").unwrap();
        assert_eq!(item["phase_slice_type"], "HITL");
    }
}
