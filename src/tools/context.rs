use schemars::JsonSchema;
use serde::Deserialize;

use crate::context::{self, VALID_SHAPES};
use crate::db::{Db, TaskQueryFilter};
use crate::error::YojanaError;

const DEFAULT_MAX_MESSAGES: usize = 10;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextArgs {
    /// Task id — UUID or "project-slug/N" (required unless arc is set)
    #[serde(default)]
    pub task: Option<String>,
    /// Arc id — UUID or "project-slug/~N" (for arc-level summary)
    #[serde(default)]
    pub arc: Option<String>,
    /// Context shape: "summary", "working", "planning", "agent", or "review"
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

    if let Some(ref arc_id) = args.arc {
        if args.shape != "summary" {
            return Err(YojanaError::InvalidInput(
                "arc parameter only supports shape 'summary'".into(),
            ));
        }
        return handle_arc_summary(db, arc_id);
    }

    let task_id = args
        .task
        .as_deref()
        .ok_or_else(|| YojanaError::InvalidInput("task or arc parameter required".into()))?;

    let task = db
        .get_task(task_id)?
        .ok_or_else(|| YojanaError::NotFound(format!("task '{task_id}'")))?;
    let edges = db.list_edges_for_task(&task.id)?;

    match args.shape.as_str() {
        "summary" => {
            let bundle = context::summary(&task, &edges);
            Ok(serde_json::to_value(bundle)?)
        }
        "working" => {
            let neighbors_with_edges = load_neighbors(db, task.id, &edges)?;
            let messages = db.get_conversation_messages(&task.id)?;
            let arc_ctx = load_arc_context(db, &task)?;
            let bundle = context::working(
                &task,
                &neighbors_with_edges,
                &messages,
                DEFAULT_MAX_MESSAGES,
                arc_ctx.as_ref().map(|c| c.info.clone()),
            );
            Ok(serde_json::to_value(bundle)?)
        }
        "planning" => {
            let neighbors_with_edges = load_neighbors(db, task.id, &edges)?;
            let messages = db.get_conversation_messages(&task.id)?;
            let arc_ctx = load_arc_context(db, &task)?;
            let (arc_info, prior_tasks) = match arc_ctx {
                Some(c) => (Some(c.info), c.prior_phase_tasks),
                None => (None, Vec::new()),
            };
            let bundle = context::planning(
                &task,
                &neighbors_with_edges,
                &messages,
                DEFAULT_MAX_MESSAGES,
                arc_info,
                &prior_tasks,
            );
            Ok(serde_json::to_value(bundle)?)
        }
        "agent" => {
            let arc_ctx = load_arc_context(db, &task)?;
            let (arc_info, prior_tasks) = match arc_ctx {
                Some(c) => (Some(c.info), c.prior_phase_tasks),
                None => (None, Vec::new()),
            };
            let bundle = context::agent(&task, arc_info, &prior_tasks);
            Ok(serde_json::to_value(bundle)?)
        }
        "review" => {
            let neighbors_with_edges = load_neighbors(db, task.id, &edges)?;
            let bundle = context::review(&task, &neighbors_with_edges);
            Ok(serde_json::to_value(bundle)?)
        }
        _ => unreachable!(),
    }
}

fn handle_arc_summary(db: &Db, arc_id: &str) -> Result<serde_json::Value, YojanaError> {
    let arc = db
        .get_arc(arc_id)?
        .ok_or_else(|| YojanaError::NotFound(format!("arc '{arc_id}'")))?;

    let human_id = format!("{}/~{}", arc.project_slug, arc.sequence_number);
    let phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases)?;

    let arc_tasks = db.list_tasks(&TaskQueryFilter {
        arc_id: Some(arc.id),
        limit: Some(500),
        ..Default::default()
    })?;

    let arc_task_ids: Vec<uuid::Uuid> = arc_tasks.iter().map(|t| t.id).collect();

    let mut all_edges = Vec::new();
    for t in &arc_tasks {
        let edges = db.list_edges_for_task(&t.id)?;
        all_edges.extend(edges);
    }

    let external_ids: Vec<uuid::Uuid> = all_edges
        .iter()
        .filter(|e| e.edge_type == "depends_on" && arc_task_ids.contains(&e.source_task_id))
        .map(|e| e.target_task_id)
        .filter(|id| !arc_task_ids.contains(id))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let mut external_tasks = Vec::new();
    for id in &external_ids {
        if let Some(t) = db.get_task(&id.to_string())? {
            external_tasks.push(t);
        }
    }

    let blockers = context::find_cross_arc_blockers(&arc_tasks, &external_tasks, &all_edges);
    let bundle = context::arc_summary(
        &human_id,
        &arc.title,
        &arc.status,
        &phases,
        &arc_tasks,
        &blockers,
    );
    Ok(serde_json::to_value(bundle)?)
}

struct ArcContext {
    info: context::ArcInfo,
    prior_phase_tasks: Vec<crate::db::TaskRow>,
}

fn load_arc_context(db: &Db, task: &crate::db::TaskRow) -> Result<Option<ArcContext>, YojanaError> {
    let (arc_id, arc_phase) = match (&task.arc_id, &task.arc_phase) {
        (Some(id), Some(phase)) => (id, phase),
        _ => return Ok(None),
    };

    let arc = db
        .get_arc(&arc_id.to_string())?
        .ok_or_else(|| YojanaError::NotFound(format!("arc '{arc_id}'")))?;

    let phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases)?;
    let total = phases.len();
    let position = phases
        .iter()
        .position(|p| p.get("name").and_then(|n| n.as_str()) == Some(arc_phase))
        .ok_or_else(|| {
            YojanaError::InvalidInput(format!(
                "task references phase '{arc_phase}' not found in arc '{arc_id}'"
            ))
        })?
        + 1;

    let prior_phase_names: Vec<&str> = phases
        .iter()
        .take_while(|p| p.get("name").and_then(|n| n.as_str()) != Some(arc_phase))
        .filter_map(|p| p.get("name").and_then(|n| n.as_str()))
        .collect();

    let prior_phase_tasks = if prior_phase_names.is_empty() {
        Vec::new()
    } else {
        db.list_tasks(&TaskQueryFilter {
            arc_id: Some(*arc_id),
            limit: Some(500),
            ..Default::default()
        })?
        .into_iter()
        .filter(|t| {
            t.arc_phase
                .as_deref()
                .map(|p| prior_phase_names.contains(&p))
                .unwrap_or(false)
        })
        .collect()
    };

    Ok(Some(ArcContext {
        info: context::ArcInfo {
            arc_title: arc.title,
            current_phase: arc_phase.clone(),
            phase_position: format!("phase {position} of {total}"),
        },
        prior_phase_tasks,
    }))
}

fn load_neighbors(
    db: &Db,
    task_id: uuid::Uuid,
    edges: &[crate::db::EdgeRow],
) -> Result<Vec<(crate::db::TaskRow, Vec<crate::db::EdgeRow>)>, YojanaError> {
    let nids = context::neighbor_ids(task_id, edges);
    let mut out = Vec::new();
    for nid in &nids {
        if let Some(ntask) = db.get_task(&nid.to_string())? {
            let nedges = db.list_edges_for_task(&ntask.id)?;
            out.push((ntask, nedges));
        }
    }
    Ok(out)
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

    fn create_arc(db: &Db) -> serde_json::Value {
        arc::handle(
            db,
            arc::ArcArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some("Test Arc".into()),
                description: None,
                status: None,
                phases: Some(vec![
                    serde_json::json!({"name": "design", "slice_type": "HITL"}),
                    serde_json::json!({"name": "implement", "slice_type": "AFK"}),
                    serde_json::json!({"name": "review", "slice_type": "HITL"}),
                ]),
                tags: None,
                context_refs: None,
                phase: None,
                note: None,
                skip: None,
            },
        )
        .unwrap()
    }

    fn create_task_in_arc(db: &Db, title: &str, arc_phase: &str) -> serde_json::Value {
        task::handle(
            db,
            task::TaskArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some(title.into()),
                description: None,
                category: None,
                status: None,
                slice_type: None,
                acceptance_criteria: None,
                decisions: Some(vec![
                    serde_json::json!({"text": format!("decision from {title}")}),
                ]),
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
                arc_id: Some("proj/~1".into()),
                arc_phase: Some(arc_phase.into()),
            },
        )
        .unwrap()
    }

    #[test]
    fn working_shape_includes_arc_info_via_tool() {
        let db = test_db();
        create_arc(&db);
        let t = create_task_in_arc(&db, "Impl task", "implement");
        let task_id = t["human_id"].as_str().unwrap();

        let out = handle(
            &db,
            ContextArgs {
                task: Some(task_id.into()),
                arc: None,
                shape: "working".into(),
            },
        )
        .unwrap();

        assert_eq!(out["arc_info"]["arc_title"], "Test Arc");
        assert_eq!(out["arc_info"]["current_phase"], "implement");
        assert_eq!(out["arc_info"]["phase_position"], "phase 2 of 3");
    }

    #[test]
    fn working_shape_no_arc_when_task_has_no_arc() {
        let db = test_db();
        task::handle(
            &db,
            task::TaskArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some("Plain task".into()),
                description: None,
                category: None,
                status: None,
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
        .unwrap();

        let out = handle(
            &db,
            ContextArgs {
                task: Some("proj/1".into()),
                arc: None,
                shape: "working".into(),
            },
        )
        .unwrap();

        assert!(out.get("arc_info").is_none());
    }

    #[test]
    fn planning_shape_includes_prior_phase_context_via_tool() {
        let db = test_db();
        create_arc(&db);
        let design_task = create_task_in_arc(&db, "Design doc", "design");
        let design_id = design_task["human_id"].as_str().unwrap();

        // Add execution record to design task
        task::handle(
            &db,
            task::TaskArgs {
                action: "update".into(),
                id: Some(design_id.into()),
                project: None,
                title: None,
                description: None,
                category: None,
                status: None,
                slice_type: None,
                acceptance_criteria: None,
                decisions: None,
                context_refs: None,
                files: None,
                tags: None,
                implementation_plan: None,
                execution_record: Some(Some("completed design review".into())),
                reproduction: None,
                root_cause: None,
                text: None,
                author: None,
                commit: None,
                arc_id: None,
                arc_phase: None,
            },
        )
        .unwrap();

        let impl_task = create_task_in_arc(&db, "Build it", "implement");
        let impl_id = impl_task["human_id"].as_str().unwrap();

        let out = handle(
            &db,
            ContextArgs {
                task: Some(impl_id.into()),
                arc: None,
                shape: "planning".into(),
            },
        )
        .unwrap();

        assert_eq!(out["shape"], "planning");
        assert!(out["arc_info"]["arc_title"].as_str().is_some());
        let prior_decisions = out["prior_phase_decisions"].as_array().unwrap();
        assert!(prior_decisions.len() >= 1);
        let prior_records = out["prior_phase_execution_records"].as_array().unwrap();
        assert_eq!(prior_records.len(), 1);
        assert_eq!(prior_records[0]["record"], "completed design review");
    }

    #[test]
    fn agent_shape_via_tool() {
        let db = test_db();
        create_arc(&db);
        create_task_in_arc(&db, "Design", "design");
        let impl_task = create_task_in_arc(&db, "Implement", "implement");
        let impl_id = impl_task["human_id"].as_str().unwrap();

        let out = handle(
            &db,
            ContextArgs {
                task: Some(impl_id.into()),
                arc: None,
                shape: "agent".into(),
            },
        )
        .unwrap();

        assert_eq!(out["shape"], "agent");
        assert_eq!(out["arc_info"]["current_phase"], "implement");
        assert!(out["prior_phase_decisions"].as_array().unwrap().len() >= 1);
    }

    #[test]
    fn arc_summary_via_tool() {
        let db = test_db();
        create_arc(&db);
        let t1 = create_task_in_arc(&db, "Design doc", "design");

        // Mark design task done (needs-triage → in-progress → done)
        let t1_id = t1["human_id"].as_str().unwrap();
        for next_status in &["in-progress", "done"] {
            task::handle(
                &db,
                task::TaskArgs {
                    action: "update".into(),
                    id: Some(t1_id.into()),
                    project: None,
                    title: None,
                    description: None,
                    category: None,
                    status: Some((*next_status).into()),
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
            .unwrap();
        }

        create_task_in_arc(&db, "Build it", "implement");

        let out = handle(
            &db,
            ContextArgs {
                task: None,
                arc: Some("proj/~1".into()),
                shape: "summary".into(),
            },
        )
        .unwrap();

        assert_eq!(out["shape"], "arc_summary");
        assert_eq!(out["title"], "Test Arc");
        assert_eq!(out["status"], "active");
        let phases = out["phases"].as_array().unwrap();
        assert_eq!(phases.len(), 3);
        assert_eq!(phases[0]["done_count"], 1);
        assert_eq!(phases[0]["total_count"], 1);
        assert_eq!(phases[1]["done_count"], 0);
        assert_eq!(phases[1]["total_count"], 1);
    }

    #[test]
    fn arc_summary_includes_cross_arc_blockers() {
        let db = test_db();
        create_arc(&db);

        // External non-arc task
        let ext = task::handle(
            &db,
            task::TaskArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some("External blocker".into()),
                description: None,
                category: None,
                status: None,
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
        .unwrap();

        let arc_task = create_task_in_arc(&db, "Blocked task", "implement");

        // Create depends_on edge from arc task to external task
        crate::tools::edge::handle(
            &db,
            crate::tools::edge::EdgeArgs {
                action: "create".into(),
                id: None,
                source: Some(arc_task["human_id"].as_str().unwrap().into()),
                target: Some(ext["human_id"].as_str().unwrap().into()),
                edge_type: Some("depends_on".into()),
                note: None,
                task: None,
            },
        )
        .unwrap();

        let out = handle(
            &db,
            ContextArgs {
                task: None,
                arc: Some("proj/~1".into()),
                shape: "summary".into(),
            },
        )
        .unwrap();

        let blockers = out["cross_arc_blockers"].as_array().unwrap();
        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0]["blocker_task"], ext["human_id"]);
        assert_eq!(blockers[0]["blocked_task"], arc_task["human_id"]);
    }

    #[test]
    fn arc_param_rejects_non_summary_shape() {
        let db = test_db();
        create_arc(&db);

        let err = handle(
            &db,
            ContextArgs {
                task: None,
                arc: Some("proj/~1".into()),
                shape: "working".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("only supports shape 'summary'"));
    }

    #[test]
    fn neither_task_nor_arc_returns_error() {
        let db = test_db();
        let err = handle(
            &db,
            ContextArgs {
                task: None,
                arc: None,
                shape: "summary".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("task or arc parameter required"));
    }
}
