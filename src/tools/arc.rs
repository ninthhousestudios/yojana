use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{ArcRow, ArcUpdates, CreateArcParams, Db, HistoryEntry};
use crate::error::YojanaError;

// Field docs omitted to keep the schema small (it reloads on summarization);
// semantics live in the tool-level description in src/mcp.rs.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ArcArgs {
    pub action: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub phases: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub context_refs: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub skip: Option<bool>,
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

/// Slim acknowledgement for arc writes (create/update/advance/revert) so they
/// don't echo the full phases array + history every call. `active_phase` is the
/// name of the currently-active phase — the field most writes care about. Use
/// action=get for full detail.
fn slim_ack(row: &ArcRow) -> serde_json::Value {
    let phases: Vec<serde_json::Value> = serde_json::from_str(&row.phases).unwrap_or_default();
    let active_phase = phases
        .iter()
        .find(|p| p.get("status").and_then(|s| s.as_str()) == Some("active"))
        .and_then(|p| p.get("name").and_then(|n| n.as_str()))
        .map(|s| s.to_string());
    serde_json::json!({
        "id": row.id.to_string(),
        "human_id": format!("{}/~{}", row.project_slug, row.sequence_number),
        "status": row.status,
        "active_phase": active_phase,
    })
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
            Ok(slim_ack(&row))
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
            Ok(slim_ack(&row))
        }
        "advance" => {
            let id = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for advance".into()))?;
            let skip = args.skip.unwrap_or(false);
            let row = db.advance_arc_phase(id, args.phase.as_deref(), skip, args.note)?;
            Ok(slim_ack(&row))
        }
        "revert" => {
            let id = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for revert".into()))?;
            let phase = args
                .phase
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("phase required for revert".into()))?;
            let row = db.revert_arc_phase(id, phase, args.note)?;
            Ok(slim_ack(&row))
        }
        other => Err(YojanaError::InvalidInput(format!(
            "unknown action '{other}'; valid: create, get, update, advance, revert"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::task;

    fn test_db() -> Db {
        let db = Db::open_in_memory().unwrap();
        db.create_project("proj", "Project", "", None).unwrap();
        db
    }

    fn create_arc(db: &Db) -> serde_json::Value {
        create_arc_with_phases(
            db,
            vec![
                serde_json::json!({"name": "design", "slice_type": "HITL"}),
                serde_json::json!({"name": "implement", "slice_type": "AFK"}),
            ],
        )
    }

    fn create_arc_with_phases(db: &Db, phases: Vec<serde_json::Value>) -> serde_json::Value {
        handle(
            db,
            ArcArgs {
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

    fn arc_args(action: &str, id: &str) -> ArcArgs {
        ArcArgs {
            action: action.into(),
            id: Some(id.into()),
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
        }
    }

    // Arc writes return a slim ack (yojana/33); fetch full detail via get.
    fn get_full(db: &Db, id: &str) -> serde_json::Value {
        handle(db, arc_args("get", id)).unwrap()
    }

    #[test]
    fn arc_tool_create_returns_slim_ack() {
        let db = test_db();
        let out = create_arc(&db);
        assert_eq!(out["human_id"], "proj/~1");
        assert_eq!(out["status"], "active");
        assert_eq!(out["active_phase"], "design");
        let full = get_full(&db, "proj/~1");
        assert_eq!(full["phases"][0]["status"], "active");
        assert_eq!(full["phases"][1]["status"], "pending");
    }

    #[test]
    fn arc_tool_get_by_sigil() {
        let db = test_db();
        create_arc(&db);
        let out = handle(&db, arc_args("get", "proj/~1")).unwrap();
        assert_eq!(out["title"], "Test Arc");
    }

    #[test]
    fn task_rejects_arc_phase_without_arc_id() {
        let db = test_db();
        let err = task::handle(
            &db,
            task::TaskArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some("Bad task".into()),
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
                arc_phase: Some("design".into()),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("arc_id required"));
    }

    #[test]
    fn task_rejects_arc_id_without_arc_phase() {
        let db = test_db();
        let arc = create_arc(&db);
        let arc_id = arc["id"].as_str().unwrap();

        let err = task::handle(
            &db,
            task::TaskArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some("Bad task".into()),
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
                arc_id: Some(arc_id.into()),
                arc_phase: None,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("arc_phase required"));
    }

    #[test]
    fn task_rejects_invalid_phase_name() {
        let db = test_db();
        let arc = create_arc(&db);
        let arc_id = arc["id"].as_str().unwrap();

        let err = task::handle(
            &db,
            task::TaskArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some("Bad task".into()),
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
                arc_id: Some(arc_id.into()),
                arc_phase: Some("nonexistent".into()),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown phase"));
    }

    #[test]
    fn task_create_with_valid_arc_assignment() {
        let db = test_db();
        let arc = create_arc(&db);

        let task = task::handle(
            &db,
            task::TaskArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some("Design doc".into()),
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
                arc_id: Some("proj/~1".into()),
                arc_phase: Some("design".into()),
            },
        )
        .unwrap();
        // create returns a slim ack (yojana/32); arc assignment is verified via get.
        let full = task::handle(
            &db,
            task::TaskArgs {
                action: "get".into(),
                id: Some(task["human_id"].as_str().unwrap().to_string()),
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
        assert_eq!(full["arc_id"], arc["id"]);
        assert_eq!(full["arc_phase"], "design");
    }

    #[test]
    fn task_create_rejects_cross_project_arc() {
        let db = test_db();
        db.create_project("other", "Other Project", "", None)
            .unwrap();
        create_arc(&db); // arc in "proj"

        let err = task::handle(
            &db,
            task::TaskArgs {
                action: "create".into(),
                id: None,
                project: Some("other".into()),
                title: Some("Cross-project".into()),
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
                arc_id: Some("proj/~1".into()),
                arc_phase: Some("design".into()),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("different project"));
    }

    #[test]
    fn task_update_rejects_cross_project_arc() {
        let db = test_db();
        db.create_project("other", "Other Project", "", None)
            .unwrap();
        create_arc(&db); // arc in "proj"

        let task = task::handle(
            &db,
            task::TaskArgs {
                action: "create".into(),
                id: None,
                project: Some("other".into()),
                title: Some("Other task".into()),
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
        let task_id = task["human_id"].as_str().unwrap().to_string();

        let err = task::handle(
            &db,
            task::TaskArgs {
                action: "update".into(),
                id: Some(task_id),
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
                execution_record: None,
                reproduction: None,
                root_cause: None,
                text: None,
                author: None,
                commit: None,
                arc_id: Some("proj/~1".into()),
                arc_phase: Some("design".into()),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("different project"));
    }

    #[test]
    fn arc_update_records_description_history() {
        let db = test_db();
        create_arc(&db);

        let mut args = arc_args("update", "proj/~1");
        args.description = Some("New description".into());
        handle(&db, args).unwrap();
        let updated = get_full(&db, "proj/~1");
        let history = updated["history"].as_array().unwrap();
        let desc_entry = history
            .iter()
            .find(|e| e["kind"] == "updated" && e["payload"]["field"] == "description");
        assert!(desc_entry.is_some());
    }

    #[test]
    fn arc_update_records_tags_history() {
        let db = test_db();
        create_arc(&db);

        let mut args = arc_args("update", "proj/~1");
        args.tags = Some(vec!["new-tag".into()]);
        handle(&db, args).unwrap();
        let updated = get_full(&db, "proj/~1");
        let history = updated["history"].as_array().unwrap();
        let tags_entry = history
            .iter()
            .find(|e| e["kind"] == "updated" && e["payload"]["field"] == "tags");
        assert!(tags_entry.is_some());
    }

    #[test]
    fn arc_update_records_context_refs_history() {
        let db = test_db();
        create_arc(&db);

        let mut args = arc_args("update", "proj/~1");
        args.context_refs = Some(vec![
            serde_json::json!({"type": "doc:path", "value": "foo.md"}),
        ]);
        handle(&db, args).unwrap();
        let updated = get_full(&db, "proj/~1");
        let history = updated["history"].as_array().unwrap();
        let refs_entry = history
            .iter()
            .find(|e| e["kind"] == "updated" && e["payload"]["field"] == "context_refs");
        assert!(refs_entry.is_some());
    }

    #[test]
    fn arc_create_rejects_invalid_phase_status() {
        let db = test_db();

        let err = handle(
            &db,
            ArcArgs {
                action: "create".into(),
                id: None,
                project: Some("proj".into()),
                title: Some("Bad Arc".into()),
                description: None,
                status: None,
                phases: Some(vec![
                    serde_json::json!({"name": "design", "status": "bogus"}),
                ]),
                tags: None,
                context_refs: None,
                phase: None,
                note: None,
                skip: None,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid phase status"));
    }

    // --- Phase advance tests ---

    #[test]
    fn advance_completes_active_phase_and_activates_next() {
        let db = test_db();
        create_arc(&db);

        handle(&db, arc_args("advance", "proj/~1")).unwrap();
        let out = get_full(&db, "proj/~1");
        let phases = out["phases"].as_array().unwrap();
        assert_eq!(phases[0]["name"], "design");
        assert_eq!(phases[0]["status"], "completed");
        assert_eq!(phases[1]["name"], "implement");
        assert_eq!(phases[1]["status"], "active");
    }

    #[test]
    fn advance_targets_specific_phase_by_name() {
        let db = test_db();
        create_arc_with_phases(
            &db,
            vec![
                serde_json::json!({"name": "design", "status": "active"}),
                serde_json::json!({"name": "implement", "status": "active"}),
                serde_json::json!({"name": "review", "status": "pending"}),
            ],
        );

        let mut args = arc_args("advance", "proj/~1");
        args.phase = Some("implement".into());
        handle(&db, args).unwrap();
        let out = get_full(&db, "proj/~1");
        let phases = out["phases"].as_array().unwrap();
        assert_eq!(phases[0]["status"], "active");
        assert_eq!(phases[1]["status"], "completed");
        assert_eq!(phases[2]["status"], "active");
    }

    #[test]
    fn advance_with_skip_sets_skipped() {
        let db = test_db();
        create_arc(&db);

        let mut args = arc_args("advance", "proj/~1");
        args.skip = Some(true);
        handle(&db, args).unwrap();
        let out = get_full(&db, "proj/~1");
        let phases = out["phases"].as_array().unwrap();
        assert_eq!(phases[0]["status"], "skipped");
        assert_eq!(phases[1]["status"], "active");
    }

    #[test]
    fn revert_sets_completed_phase_back_to_active() {
        let db = test_db();
        create_arc(&db);
        handle(&db, arc_args("advance", "proj/~1")).unwrap();

        let mut args = arc_args("revert", "proj/~1");
        args.phase = Some("design".into());
        handle(&db, args).unwrap();
        let out = get_full(&db, "proj/~1");
        let phases = out["phases"].as_array().unwrap();
        assert_eq!(phases[0]["status"], "active");
        assert_eq!(phases[1]["status"], "active");
    }

    #[test]
    fn revert_does_not_cascade() {
        let db = test_db();
        create_arc_with_phases(
            &db,
            vec![
                serde_json::json!({"name": "design"}),
                serde_json::json!({"name": "implement"}),
                serde_json::json!({"name": "review"}),
            ],
        );
        handle(&db, arc_args("advance", "proj/~1")).unwrap();
        handle(&db, arc_args("advance", "proj/~1")).unwrap();

        let mut args = arc_args("revert", "proj/~1");
        args.phase = Some("design".into());
        handle(&db, args).unwrap();
        let out = get_full(&db, "proj/~1");
        let phases = out["phases"].as_array().unwrap();
        assert_eq!(phases[0]["status"], "active");
        assert_eq!(phases[1]["status"], "completed");
        assert_eq!(phases[2]["status"], "active");
    }

    #[test]
    fn phase_transitions_recorded_in_history_with_note() {
        let db = test_db();
        create_arc(&db);

        let mut args = arc_args("advance", "proj/~1");
        args.note = Some("design review passed".into());
        handle(&db, args).unwrap();
        let out = get_full(&db, "proj/~1");
        let history = out["history"].as_array().unwrap();
        let entry = history
            .iter()
            .find(|e| e["kind"] == "phase_advanced")
            .unwrap();
        assert_eq!(entry["payload"]["phase"], "design");
        assert_eq!(entry["payload"]["to"], "completed");
        assert_eq!(entry["payload"]["note"], "design review passed");
        assert!(entry["ts"].as_i64().unwrap() > 0);
    }

    #[test]
    fn advance_past_last_phase_does_not_error() {
        let db = test_db();
        create_arc_with_phases(&db, vec![serde_json::json!({"name": "only-phase"})]);

        handle(&db, arc_args("advance", "proj/~1")).unwrap();
        let out = get_full(&db, "proj/~1");
        let phases = out["phases"].as_array().unwrap();
        assert_eq!(phases[0]["status"], "completed");
    }

    #[test]
    fn cannot_advance_completed_phase() {
        let db = test_db();
        create_arc(&db);
        handle(&db, arc_args("advance", "proj/~1")).unwrap();

        let mut args = arc_args("advance", "proj/~1");
        args.phase = Some("design".into());
        let err = handle(&db, args).unwrap_err();
        assert!(err.to_string().contains("cannot advance"));
        assert!(err.to_string().contains("completed"));
    }

    #[test]
    fn cannot_advance_skipped_phase() {
        let db = test_db();
        create_arc(&db);
        let mut args = arc_args("advance", "proj/~1");
        args.skip = Some(true);
        handle(&db, args).unwrap();

        let mut args = arc_args("advance", "proj/~1");
        args.phase = Some("design".into());
        let err = handle(&db, args).unwrap_err();
        assert!(err.to_string().contains("cannot advance"));
        assert!(err.to_string().contains("skipped"));
    }

    #[test]
    fn cannot_revert_non_completed_phase() {
        let db = test_db();
        create_arc(&db);

        let mut args = arc_args("revert", "proj/~1");
        args.phase = Some("design".into());
        let err = handle(&db, args).unwrap_err();
        assert!(err.to_string().contains("cannot revert"));
        assert!(err.to_string().contains("active"));
    }

    #[test]
    fn revert_requires_phase_name() {
        let db = test_db();
        create_arc(&db);

        let err = handle(&db, arc_args("revert", "proj/~1")).unwrap_err();
        assert!(err.to_string().contains("phase required"));
    }
}
