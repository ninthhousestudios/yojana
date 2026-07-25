use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::db::{CreateTaskParams, Db, HistoryEntry, TaskRow, TaskUpdates};
use crate::error::YojanaError;
use crate::tools::acceptance::{self, AcceptanceCriterionInput};
use crate::tools::context_ref::{ContextRef, RefType};

fn deserialize_double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TaskAction {
    Create,
    Get,
    Update,
    Comment,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TaskCategory {
    Bug,
    Enhancement,
    Experiment,
}

impl AsRef<str> for TaskCategory {
    fn as_ref(&self) -> &str {
        match self {
            Self::Bug => "bug",
            Self::Enhancement => "enhancement",
            Self::Experiment => "experiment",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
pub enum SliceType {
    #[serde(rename = "AFK", alias = "afk")]
    Afk,
    #[serde(rename = "HITL", alias = "hitl")]
    Hitl,
}

impl AsRef<str> for SliceType {
    fn as_ref(&self) -> &str {
        match self {
            Self::Afk => "AFK",
            Self::Hitl => "HITL",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    NeedsTriage,
    NeedsInfo,
    ReadyForAgent,
    ReadyForHuman,
    InProgress,
    NeedsReview,
    Done,
    Wontfix,
}

impl AsRef<str> for TaskStatus {
    fn as_ref(&self) -> &str {
        match self {
            Self::NeedsTriage => "needs-triage",
            Self::NeedsInfo => "needs-info",
            Self::ReadyForAgent => "ready-for-agent",
            Self::ReadyForHuman => "ready-for-human",
            Self::InProgress => "in-progress",
            Self::NeedsReview => "needs-review",
            Self::Done => "done",
            Self::Wontfix => "wontfix",
        }
    }
}

// Field-level docs are intentionally omitted: schemars serializes them into
// the tool schema, which reloads several times per long session. Cross-cutting
// semantics (null-clears, arc pairing, commit shorthand, comment fields) live
// once in the tool-level description in src/mcp.rs.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskArgs {
    pub action: TaskAction,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub category: Option<Option<TaskCategory>>,
    #[serde(default)]
    pub status: Option<TaskStatus>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub slice_type: Option<Option<SliceType>>,
    #[serde(default)]
    pub acceptance_criteria: Option<Vec<AcceptanceCriterionInput>>,
    #[serde(default)]
    pub decisions: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub context_refs: Option<Vec<ContextRef>>,
    #[serde(default)]
    pub files: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub implementation_plan: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub execution_record: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub reproduction: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub root_cause: Option<Option<String>>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub arc_id: Option<String>,
    #[serde(default)]
    pub arc_phase: Option<String>,
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
    pub context_refs: Vec<ContextRef>,
    pub files: Vec<String>,
    pub tags: Vec<String>,
    pub history: Vec<HistoryEntry>,
    pub messages: Vec<serde_json::Value>,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc_phase: Option<String>,
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
            context_refs: ContextRef::parse_array(&row.context_refs),
            files: serde_json::from_str(&row.files).unwrap_or_default(),
            tags: serde_json::from_str(&row.tags).unwrap_or_default(),
            history: serde_json::from_str(&row.history).unwrap_or_default(),
            messages: Vec::new(),
            created_at: row.created_at,
            updated_at: row.updated_at,
            completed_at: row.completed_at,
            arc_id: row.arc_id.map(|id| id.to_string()),
            arc_phase: row.arc_phase,
        }
    }
}

fn json_array(s: &str) -> Vec<serde_json::Value> {
    serde_json::from_str(s).unwrap_or_default()
}

/// Slim acknowledgement returned by create/update so the common write path
/// doesn't echo the full task object back every call. Callers needing detail
/// fetch it via action=get (or yojana_context).
fn slim_ack(row: &TaskRow) -> serde_json::Value {
    serde_json::json!({
        "id": row.id.to_string(),
        "human_id": format!("{}/{}", row.project_slug, row.sequence_number),
        "status": row.status,
        "title": row.title,
    })
}

fn to_json(val: &(impl serde::Serialize + ?Sized)) -> Result<String, YojanaError> {
    serde_json::to_string(val).map_err(YojanaError::Json)
}

fn resolve_arc_assignment(
    db: &Db,
    arc_id: Option<&str>,
    arc_phase: Option<&str>,
    project_id: &Uuid,
) -> Result<(Option<Uuid>, Option<String>), YojanaError> {
    match (arc_id, arc_phase) {
        (None, None) => Ok((None, None)),
        (Some(_), None) => Err(YojanaError::InvalidInput(
            "arc_phase required when arc_id is provided".into(),
        )),
        (None, Some(_)) => Err(YojanaError::InvalidInput(
            "arc_id required when arc_phase is provided".into(),
        )),
        (Some(arc_str), Some(phase)) => {
            let (uuid, arc_project_id) = db.resolve_arc_id(arc_str)?;
            if arc_project_id != *project_id {
                return Err(YojanaError::InvalidInput(
                    "arc belongs to a different project than the task".into(),
                ));
            }
            db.validate_task_arc(&uuid, phase)?;
            Ok((Some(uuid), Some(phase.to_string())))
        }
    }
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
    match args.action {
        TaskAction::Create => {
            let project = args
                .project
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("project required for create".into()))?;
            let title = args
                .title
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("title required for create".into()))?;
            let refs = args.context_refs.as_deref().unwrap_or(&[]);
            let (project_id, project_slug) = resolve_project(db, project)?;
            let (arc_uuid, arc_phase) = resolve_arc_assignment(
                db,
                args.arc_id.as_deref(),
                args.arc_phase.as_deref(),
                &project_id,
            )?;

            let params = CreateTaskParams {
                project_id,
                project_slug,
                title: title.to_string(),
                description: args.description.unwrap_or_default(),
                category: args.category.flatten().map(|c| c.as_ref().to_string()),
                status: args.status.map(|s| s.as_ref().to_string()),
                slice_type: args.slice_type.flatten().map(|s| s.as_ref().to_string()),
                acceptance_criteria: to_json(&acceptance::normalize(
                    args.acceptance_criteria.unwrap_or_default(),
                ))?,
                decisions: to_json(&args.decisions.unwrap_or_default())?,
                context_refs: to_json(refs)?,
                files: to_json(&args.files.unwrap_or_default())?,
                tags: to_json(&args.tags.unwrap_or_default())?,
                implementation_plan: args.implementation_plan.flatten(),
                execution_record: args.execution_record.flatten(),
                reproduction: args.reproduction.flatten(),
                root_cause: args.root_cause.flatten(),
                arc_id: arc_uuid,
                arc_phase,
            };
            let actor = args.author.as_deref().unwrap_or("agent");
            let row = db.create_task(params, actor)?;
            Ok(slim_ack(&row))
        }
        TaskAction::Get => {
            let id = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for get".into()))?;
            let row = db
                .get_task(id)?
                .ok_or_else(|| YojanaError::NotFound(format!("task '{id}'")))?;
            let task_id = row.id;
            let mut output = TaskOutput::from(row);
            output.messages = db.get_conversation_messages(&task_id)?;
            Ok(serde_json::to_value(output)?)
        }
        TaskAction::Update => {
            let id = args
                .id
                .as_deref()
                .ok_or_else(|| YojanaError::InvalidInput("id required for update".into()))?;
            let (arc_id_update, arc_phase_update) =
                match (args.arc_id.as_deref(), args.arc_phase.as_deref()) {
                    (None, None) => (None, None),
                    (Some(_), None) => {
                        return Err(YojanaError::InvalidInput(
                            "arc_phase required when arc_id is provided".into(),
                        ));
                    }
                    (None, Some(_)) => {
                        return Err(YojanaError::InvalidInput(
                            "arc_id required when arc_phase is provided".into(),
                        ));
                    }
                    (Some(arc_str), Some(phase)) => {
                        let (uuid, arc_project_id) = db.resolve_arc_id(arc_str)?;
                        let task_row = db
                            .get_task(id)?
                            .ok_or_else(|| YojanaError::NotFound(format!("task '{id}'")))?;
                        if arc_project_id != task_row.project_id {
                            return Err(YojanaError::InvalidInput(
                                "arc belongs to a different project than the task".into(),
                            ));
                        }
                        db.validate_task_arc(&uuid, phase)?;
                        (Some(Some(uuid)), Some(Some(phase.to_string())))
                    }
                };
            // If a commit shorthand is provided, append it as a git:commit
            // context_ref. Combines with any explicit context_refs the caller
            // also passed (those take precedence as the base list).
            let merged_context_refs = if let Some(ref sha) = args.commit {
                let mut refs: Vec<ContextRef> = match args.context_refs {
                    Some(ref existing) => existing.clone(),
                    None => {
                        let task = db
                            .get_task(id)?
                            .ok_or_else(|| YojanaError::NotFound(format!("task '{id}'")))?;
                        ContextRef::parse_array(&task.context_refs)
                    }
                };
                refs.push(ContextRef {
                    ref_type: RefType::GitCommit,
                    value: sha.clone(),
                    label: None,
                });
                Some(refs)
            } else {
                args.context_refs
            };
            let has_status_request = args.status.is_some();
            let old_status = if has_status_request {
                db.get_task(id)?.map(|t| t.status)
            } else {
                None
            };
            let updates = TaskUpdates {
                title: args.title,
                description: args.description,
                category: args.category.map(|o| o.map(|c| c.as_ref().to_string())),
                status: args.status.map(|s| s.as_ref().to_string()),
                force_status: false,
                slice_type: args.slice_type.map(|o| o.map(|s| s.as_ref().to_string())),
                acceptance_criteria: args
                    .acceptance_criteria
                    .map(|v| to_json(&acceptance::normalize(v)))
                    .transpose()?,
                decisions: args.decisions.map(|v| to_json(&v)).transpose()?,
                implementation_plan: args.implementation_plan,
                execution_record: args.execution_record,
                reproduction: args.reproduction,
                root_cause: args.root_cause,
                context_refs: merged_context_refs.map(|v| to_json(&v)).transpose()?,
                files: args.files.map(|v| to_json(&v)).transpose()?,
                tags: args.tags.map(|v| to_json(&v)).transpose()?,
                arc_id: arc_id_update,
                arc_phase: arc_phase_update,
            };
            let actor = args.author.as_deref().unwrap_or("agent");
            let row = db.update_task(id, updates, actor)?;
            if let Some(old) = old_status
                && old != row.status
            {
                db.try_auto_advance_phase(&row.id, actor)?;
            }
            Ok(slim_ack(&row))
        }
        TaskAction::Comment => {
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
            let message = db.append_conversation_message(&task.id, text, args.author.as_deref())?;
            Ok(serde_json::json!({
                "task": format!("{}/{}", task.project_slug, task.sequence_number),
                "ts": message.get("ts"),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::arc;

    #[test]
    fn schema_stays_under_token_budget() {
        let schema = schemars::schema_for!(TaskArgs);
        let json = serde_json::to_string(&schema).unwrap();
        let est_tokens = json.len() / 4;
        eprintln!(
            "TaskArgs schema: {} chars (~{} tokens)",
            json.len(),
            est_tokens
        );
        // Guards the yojana/32 schema diet: the hot-path schema reloads several
        // times per long session, so keep it lean. Budget raised from 450→550
        // in yojana/34 to accommodate the typed ContextRef item schema, and
        // 550→620 in yojana/42 for the typed acceptance_criteria item schema
        // (the untyped `items: true` was what let two storage shapes in).
        assert!(
            est_tokens <= 620,
            "TaskArgs schema grew to ~{est_tokens} tokens (>620); did a field description sneak back in?"
        );
    }

    fn plain_blank(action: TaskAction) -> TaskArgs {
        TaskArgs {
            action,
            id: None,
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
        }
    }

    fn keys(v: &serde_json::Value) -> Vec<String> {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    }

    #[test]
    fn create_returns_slim_ack() {
        let db = test_db();
        let mut args = task_args_create("Slim me", "design");
        args.arc_id = None;
        args.arc_phase = None;
        let out = handle(&db, args).unwrap();
        assert_eq!(keys(&out), vec!["human_id", "id", "status", "title"]);
        assert_eq!(out["title"], "Slim me");
    }

    #[test]
    fn update_returns_slim_ack() {
        let db = test_db();
        let mut args = task_args_create("Edit me", "design");
        args.arc_id = None;
        args.arc_phase = None;
        let created = handle(&db, args).unwrap();
        let id = created["human_id"].as_str().unwrap().to_string();
        let out = update_status(&db, &id, TaskStatus::InProgress);
        assert_eq!(keys(&out), vec!["human_id", "id", "status", "title"]);
        assert_eq!(out["status"], "in-progress");
    }

    #[test]
    fn comment_omits_echoed_text() {
        let db = test_db();
        let mut args = task_args_create("Talk to me", "design");
        args.arc_id = None;
        args.arc_phase = None;
        let created = handle(&db, args).unwrap();
        let id = created["human_id"].as_str().unwrap().to_string();
        let out = handle(
            &db,
            TaskArgs {
                action: TaskAction::Comment,
                id: Some(id),
                text: Some("a fairly long comment body that we do not want echoed back".into()),
                ..plain_blank(TaskAction::Comment)
            },
        )
        .unwrap();
        assert_eq!(keys(&out), vec!["task", "ts"]);
        assert!(out.get("message").is_none());
    }

    #[test]
    fn get_returns_full_detail() {
        let db = test_db();
        let mut args = task_args_create("Full me", "design");
        args.arc_id = None;
        args.arc_phase = None;
        let created = handle(&db, args).unwrap();
        let id = created["human_id"].as_str().unwrap().to_string();
        let out = handle(
            &db,
            TaskArgs {
                action: TaskAction::Get,
                id: Some(id),
                ..plain_blank(TaskAction::Get)
            },
        )
        .unwrap();
        // get must still surface the heavy fields (no regression for review/planning).
        for field in [
            "description",
            "history",
            "messages",
            "acceptance_criteria",
            "tags",
        ] {
            assert!(out.get(field).is_some(), "get should include {field}");
        }
    }

    fn test_db() -> Db {
        let db = Db::open_in_memory().unwrap();
        db.create_project("proj", "Project", "", None, "test")
            .unwrap();
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
                    serde_json::json!({"name": "design", "gate": "auto"}),
                    serde_json::json!({"name": "implement", "gate": "auto"}),
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

    fn task_args_create(title: &str, phase: &str) -> TaskArgs {
        TaskArgs {
            project: Some("proj".into()),
            title: Some(title.into()),
            arc_id: Some("proj/~1".into()),
            arc_phase: Some(phase.into()),
            ..plain_blank(TaskAction::Create)
        }
    }

    fn update_status(db: &Db, id: &str, status: TaskStatus) -> serde_json::Value {
        handle(
            db,
            TaskArgs {
                id: Some(id.into()),
                status: Some(status),
                ..plain_blank(TaskAction::Update)
            },
        )
        .unwrap()
    }

    #[test]
    fn auto_advance_via_task_update_tool() {
        let db = test_db();
        create_arc(&db);
        let t = handle(&db, task_args_create("Design doc", "design")).unwrap();
        let tid = t["human_id"].as_str().unwrap();

        update_status(&db, tid, TaskStatus::InProgress);
        update_status(&db, tid, TaskStatus::Done);

        let arc = db.get_arc("proj/~1").unwrap().unwrap();
        let phases: Vec<serde_json::Value> = serde_json::from_str(&arc.phases).unwrap();
        assert_eq!(
            phases[0]["status"], "completed",
            "design should auto-advance to completed"
        );
        assert_eq!(
            phases[1]["status"], "active",
            "implement should become active"
        );

        let history: Vec<crate::db::HistoryEntry> = serde_json::from_str(&arc.history).unwrap();
        let auto = history.iter().find(|e| e.kind == "phase_auto_advanced");
        assert!(auto.is_some(), "should log phase_auto_advanced");
        assert_eq!(auto.unwrap().payload["trigger_task"], tid);
    }
}
