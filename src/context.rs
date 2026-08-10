use std::collections::HashMap;

use serde::Serialize;
use uuid::Uuid;

use crate::acceptance;
use crate::context_ref::{ContextRef, RefType};
use crate::db::{EdgeRow, HistoryEntry, TaskRow};
use crate::state::TaskStatus;

pub const VALID_SHAPES: &[&str] = &["summary", "working", "planning", "agent", "review"];

#[derive(Debug, Serialize, Clone)]
pub struct ArcInfo {
    pub arc_title: String,
    pub current_phase: String,
    pub phase_position: String,
}

#[derive(Debug, Serialize)]
pub struct SummaryBundle {
    pub shape: &'static str,
    pub human_id: String,
    pub title: String,
    pub status: TaskStatus,
    pub slice_type: Option<String>,
    pub category: Option<String>,
    pub edge_counts: HashMap<String, usize>,
    pub last_history: Option<HistoryEntry>,
}

#[derive(Debug, Serialize)]
pub struct WorkingBundle {
    pub shape: &'static str,
    pub human_id: String,
    pub acceptance_criteria: Vec<serde_json::Value>,
    pub decisions: Vec<serde_json::Value>,
    pub neighbors: Vec<SummaryBundle>,
    pub recent_messages: Vec<serde_json::Value>,
    pub context_refs: Vec<ContextRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc_info: Option<ArcInfo>,
}

fn human_id(task: &TaskRow) -> String {
    format!("{}/{}", task.project_slug, task.sequence_number)
}

fn count_edges(task_id: Uuid, edges: &[EdgeRow]) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for edge in edges {
        if edge.source_task_id == task_id {
            let key = format!("{}_out", edge.edge_type);
            *counts.entry(key).or_default() += 1;
        }
        if edge.target_task_id == task_id {
            let key = format!("{}_in", edge.edge_type);
            *counts.entry(key).or_default() += 1;
        }
    }
    counts
}

fn last_history(task: &TaskRow) -> Option<HistoryEntry> {
    let entries: Vec<HistoryEntry> = match serde_json::from_str(&task.history) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("corrupt JSON in history: {e}");
            return None;
        }
    };
    entries.into_iter().last()
}

fn json_array(s: &str) -> Vec<serde_json::Value> {
    match serde_json::from_str(s) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("corrupt JSON in field: {e}");
            Vec::new()
        }
    }
}

pub fn summary(task: &TaskRow, edges: &[EdgeRow]) -> SummaryBundle {
    SummaryBundle {
        shape: "summary",
        human_id: human_id(task),
        title: task.title.clone(),
        status: task.status.clone(),
        slice_type: task.slice_type.clone(),
        category: task.category.clone(),
        edge_counts: count_edges(task.id, edges),
        last_history: last_history(task),
    }
}

pub fn working(
    task: &TaskRow,
    neighbors_with_edges: &[(TaskRow, Vec<EdgeRow>)],
    messages: &[serde_json::Value],
    max_messages: usize,
    arc_info: Option<ArcInfo>,
) -> WorkingBundle {
    let neighbor_summaries: Vec<SummaryBundle> = neighbors_with_edges
        .iter()
        .map(|(t, e)| summary(t, e))
        .collect();

    let recent: Vec<serde_json::Value> = messages
        .iter()
        .skip(messages.len().saturating_sub(max_messages))
        .cloned()
        .collect();

    WorkingBundle {
        shape: "working",
        human_id: human_id(task),
        acceptance_criteria: acceptance::parse_stored_values(&task.acceptance_criteria),
        decisions: json_array(&task.decisions),
        neighbors: neighbor_summaries,
        recent_messages: recent,
        context_refs: ContextRef::parse_array(&task.context_refs),
        arc_info,
    }
}

#[derive(Debug, Serialize)]
pub struct PlanningBundle {
    pub shape: &'static str,
    pub human_id: String,
    pub acceptance_criteria: Vec<serde_json::Value>,
    pub decisions: Vec<serde_json::Value>,
    pub neighbors: Vec<SummaryBundle>,
    pub recent_messages: Vec<serde_json::Value>,
    pub context_refs: Vec<ContextRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc_info: Option<ArcInfo>,
    pub prior_phase_decisions: Vec<serde_json::Value>,
    pub prior_phase_execution_records: Vec<serde_json::Value>,
}

fn extract_prior_phase_context(
    tasks: &[TaskRow],
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let mut decisions = Vec::new();
    let mut records = Vec::new();
    for t in tasks {
        for d in json_array(&t.decisions) {
            decisions.push(d);
        }
        if let Some(ref record) = t.execution_record {
            records.push(serde_json::json!({
                "task": human_id(t),
                "phase": t.arc_phase.as_deref().unwrap_or(""),
                "record": record,
            }));
        }
    }
    (decisions, records)
}

pub fn planning(
    task: &TaskRow,
    neighbors_with_edges: &[(TaskRow, Vec<EdgeRow>)],
    messages: &[serde_json::Value],
    max_messages: usize,
    arc_info: Option<ArcInfo>,
    prior_phase_tasks: &[TaskRow],
) -> PlanningBundle {
    let neighbor_summaries: Vec<SummaryBundle> = neighbors_with_edges
        .iter()
        .map(|(t, e)| summary(t, e))
        .collect();

    let recent: Vec<serde_json::Value> = messages
        .iter()
        .skip(messages.len().saturating_sub(max_messages))
        .cloned()
        .collect();

    let (prior_decisions, prior_records) = extract_prior_phase_context(prior_phase_tasks);

    PlanningBundle {
        shape: "planning",
        human_id: human_id(task),
        acceptance_criteria: acceptance::parse_stored_values(&task.acceptance_criteria),
        decisions: json_array(&task.decisions),
        neighbors: neighbor_summaries,
        recent_messages: recent,
        context_refs: ContextRef::parse_array(&task.context_refs),
        arc_info,
        prior_phase_decisions: prior_decisions,
        prior_phase_execution_records: prior_records,
    }
}

#[derive(Debug, Serialize)]
pub struct AgentBundle {
    pub shape: &'static str,
    pub human_id: String,
    pub acceptance_criteria: Vec<serde_json::Value>,
    pub decisions: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementation_plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc_info: Option<ArcInfo>,
    pub prior_phase_decisions: Vec<serde_json::Value>,
    pub prior_phase_execution_records: Vec<serde_json::Value>,
}

pub fn agent(
    task: &TaskRow,
    arc_info: Option<ArcInfo>,
    prior_phase_tasks: &[TaskRow],
) -> AgentBundle {
    let (prior_decisions, prior_records) = extract_prior_phase_context(prior_phase_tasks);

    AgentBundle {
        shape: "agent",
        human_id: human_id(task),
        acceptance_criteria: acceptance::parse_stored_values(&task.acceptance_criteria),
        decisions: json_array(&task.decisions),
        implementation_plan: task.implementation_plan.clone(),
        arc_info,
        prior_phase_decisions: prior_decisions,
        prior_phase_execution_records: prior_records,
    }
}

#[derive(Debug, Serialize)]
pub struct ReviewBundle {
    pub shape: &'static str,
    pub human_id: String,
    pub title: String,
    pub status: TaskStatus,
    pub description: String,
    pub acceptance_criteria: Vec<serde_json::Value>,
    pub decisions: Vec<serde_json::Value>,
    pub implementation_plan: Option<String>,
    pub git_refs: Vec<ContextRef>,
    pub doc_refs: Vec<ContextRef>,
    pub other_refs: Vec<ContextRef>,
    pub neighbors: Vec<SummaryBundle>,
}

pub fn review(task: &TaskRow, neighbors_with_edges: &[(TaskRow, Vec<EdgeRow>)]) -> ReviewBundle {
    let all_refs = ContextRef::parse_array(&task.context_refs);
    let mut git_refs = Vec::new();
    let mut doc_refs = Vec::new();
    let mut other_refs = Vec::new();

    for r in all_refs {
        match r.ref_type {
            RefType::GitCommit | RefType::GitRange => git_refs.push(r),
            RefType::DocPath => doc_refs.push(r),
            _ => other_refs.push(r),
        }
    }

    let neighbor_summaries: Vec<SummaryBundle> = neighbors_with_edges
        .iter()
        .map(|(t, e)| summary(t, e))
        .collect();

    ReviewBundle {
        shape: "review",
        human_id: human_id(task),
        title: task.title.clone(),
        status: task.status.clone(),
        description: task.description.clone(),
        acceptance_criteria: acceptance::parse_stored_values(&task.acceptance_criteria),
        decisions: json_array(&task.decisions),
        implementation_plan: task.implementation_plan.clone(),
        git_refs,
        doc_refs,
        other_refs,
        neighbors: neighbor_summaries,
    }
}

#[derive(Debug, Serialize)]
pub struct ArcSummaryBundle {
    pub shape: &'static str,
    pub human_id: String,
    pub title: String,
    pub status: String,
    pub phases: Vec<serde_json::Value>,
    pub cross_arc_blockers: Vec<serde_json::Value>,
}

pub fn arc_summary(
    human_id: &str,
    title: &str,
    status: &str,
    phases: &[serde_json::Value],
    arc_tasks: &[TaskRow],
    cross_arc_blockers: &[serde_json::Value],
) -> ArcSummaryBundle {
    let enriched_phases: Vec<serde_json::Value> = phases
        .iter()
        .map(|phase| {
            let phase_name = phase.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let phase_status = phase
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("pending");

            let phase_tasks: Vec<&TaskRow> = arc_tasks
                .iter()
                .filter(|t| t.arc_phase.as_deref() == Some(phase_name))
                .collect();
            let total = phase_tasks.len();
            let done = phase_tasks
                .iter()
                .filter(|t| t.status.is_terminal())
                .count();

            serde_json::json!({
                "name": phase_name,
                "status": phase_status,
                "done_count": done,
                "total_count": total,
            })
        })
        .collect();

    ArcSummaryBundle {
        shape: "arc_summary",
        human_id: human_id.to_string(),
        title: title.to_string(),
        status: status.to_string(),
        phases: enriched_phases,
        cross_arc_blockers: cross_arc_blockers.to_vec(),
    }
}

pub fn find_cross_arc_blockers(
    arc_tasks: &[TaskRow],
    external_tasks: &[TaskRow],
    edges: &[EdgeRow],
) -> Vec<serde_json::Value> {
    let arc_task_ids: Vec<Uuid> = arc_tasks.iter().map(|t| t.id).collect();
    let terminal = ["done", "wontfix"];

    let mut blockers = Vec::new();
    for edge in edges {
        if edge.edge_type != "depends_on" {
            continue;
        }
        if !arc_task_ids.contains(&edge.source_task_id) {
            continue;
        }
        if arc_task_ids.contains(&edge.target_task_id) {
            continue;
        }
        if let Some(ext) = external_tasks
            .iter()
            .find(|t| t.id == edge.target_task_id)
            .filter(|t| !terminal.contains(&t.status.as_str()))
        {
            let blocked = arc_tasks.iter().find(|t| t.id == edge.source_task_id);
            blockers.push(serde_json::json!({
                "blocker_task": human_id(ext),
                "blocker_status": ext.status,
                "blocked_task": blocked.map(human_id).unwrap_or_default(),
            }));
        }
    }
    blockers
}

pub fn neighbor_ids(task_id: Uuid, edges: &[EdgeRow]) -> Vec<Uuid> {
    let mut ids = Vec::new();
    for edge in edges {
        if edge.source_task_id == task_id && !ids.contains(&edge.target_task_id) {
            ids.push(edge.target_task_id);
        }
        if edge.target_task_id == task_id && !ids.contains(&edge.source_task_id) {
            ids.push(edge.source_task_id);
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: u8, slug: &str, seq: i64) -> TaskRow {
        TaskRow {
            id: Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, id]),
            project_id: Uuid::from_bytes([0; 16]),
            project_slug: slug.to_string(),
            sequence_number: seq,
            title: format!("Task {id}"),
            description: String::new(),
            category: Some("enhancement".into()),
            status: TaskStatus::ReadyForAgent,
            slice_type: Some("AFK".into()),
            acceptance_criteria: r#"[{"text":"it works"}]"#.into(),
            decisions: r#"[{"text":"use X"}]"#.into(),
            implementation_plan: None,
            execution_record: None,
            reproduction: None,
            root_cause: None,
            context_refs: r#"[{"type":"git:commit","value":"abc"}]"#.into(),
            files: "[]".into(),
            tags: "[]".into(),
            history: r#"[{"ts":1000,"kind":"task_created","payload":{}},{"ts":2000,"kind":"status_changed","payload":{"from":"needs-triage","to":"ready-for-agent"}}]"#.into(),
            created_at: 1000,
            updated_at: 2000,
            completed_at: None,
            arc_id: None,
            arc_phase: None,
        }
    }

    fn make_edge(src: u8, tgt: u8, edge_type: &str) -> EdgeRow {
        EdgeRow {
            id: Uuid::now_v7(),
            source_task_id: Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, src]),
            target_task_id: Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, tgt]),
            edge_type: edge_type.to_string(),
            note: None,
            created_at: 1000,
        }
    }

    #[test]
    fn summary_shape_basic() {
        let task = make_task(1, "proj", 1);
        let edges = vec![
            make_edge(1, 2, "depends_on"),
            make_edge(1, 3, "depends_on"),
            make_edge(4, 1, "relates_to"),
        ];

        let bundle = summary(&task, &edges);
        assert_eq!(bundle.shape, "summary");
        assert_eq!(bundle.human_id, "proj/1");
        assert_eq!(bundle.title, "Task 1");
        assert_eq!(bundle.status, TaskStatus::ReadyForAgent);
        assert_eq!(bundle.slice_type.as_deref(), Some("AFK"));
        assert_eq!(bundle.category.as_deref(), Some("enhancement"));
        assert_eq!(bundle.edge_counts.get("depends_on_out"), Some(&2));
        assert_eq!(bundle.edge_counts.get("relates_to_in"), Some(&1));
        assert!(bundle.last_history.is_some());
        assert_eq!(bundle.last_history.unwrap().kind, "status_changed");
    }

    #[test]
    fn summary_no_edges_no_history() {
        let mut task = make_task(1, "proj", 1);
        task.history = "[]".into();
        let bundle = summary(&task, &[]);
        assert!(bundle.edge_counts.is_empty());
        assert!(bundle.last_history.is_none());
    }

    /// A criteria column we cannot parse must be distinguishable from an empty
    /// one on every bundle shape — an agent reads these instead of the CLI.
    #[test]
    fn unreadable_criteria_never_look_empty_on_any_shape() {
        // A top-level non-array is the shape that used to vanish entirely:
        // json_array's unwrap_or_default turned it into an empty list.
        let mut task = make_task(1, "proj", 1);
        task.acceptance_criteria = r#"{"text":"not an array"}"#.into();

        let mut empty = make_task(2, "proj", 2);
        empty.acceptance_criteria = "[]".into();

        let marker = |criteria: &[serde_json::Value]| {
            assert_eq!(criteria.len(), 1, "expected a marker element");
            assert!(
                criteria[0].get(acceptance::UNREADABLE_KEY).is_some(),
                "expected the unreadable marker, got {:?}",
                criteria[0]
            );
        };

        marker(&working(&task, &[], &[], 10, None).acceptance_criteria);
        marker(&planning(&task, &[], &[], 10, None, &[]).acceptance_criteria);
        marker(&agent(&task, None, &[]).acceptance_criteria);
        marker(&review(&task, &[]).acceptance_criteria);

        assert!(
            working(&empty, &[], &[], 10, None)
                .acceptance_criteria
                .is_empty(),
            "a genuinely empty column stays empty"
        );
    }

    #[test]
    fn working_shape_with_neighbors() {
        let task = make_task(1, "proj", 1);
        let neighbor = make_task(2, "proj", 2);
        let neighbor_edges = vec![make_edge(1, 2, "depends_on")];

        let messages = vec![
            serde_json::json!({"ts": 1000, "text": "hello"}),
            serde_json::json!({"ts": 2000, "text": "world"}),
        ];

        let bundle = working(&task, &[(neighbor, neighbor_edges)], &messages, 10, None);
        assert_eq!(bundle.shape, "working");
        assert_eq!(bundle.human_id, "proj/1");
        assert_eq!(bundle.acceptance_criteria.len(), 1);
        assert_eq!(bundle.decisions.len(), 1);
        assert_eq!(bundle.neighbors.len(), 1);
        assert_eq!(bundle.neighbors[0].human_id, "proj/2");
        assert_eq!(bundle.recent_messages.len(), 2);
        assert_eq!(bundle.context_refs.len(), 1);
    }

    #[test]
    fn working_truncates_messages() {
        let task = make_task(1, "proj", 1);
        let messages: Vec<serde_json::Value> = (0..20)
            .map(|i| serde_json::json!({"ts": i, "text": format!("msg {i}")}))
            .collect();

        let bundle = working(&task, &[], &messages, 5, None);
        assert_eq!(bundle.recent_messages.len(), 5);
        assert_eq!(bundle.recent_messages[0]["ts"], 15);
    }

    #[test]
    fn working_shape_includes_arc_info() {
        let mut task = make_task(1, "proj", 1);
        task.arc_id = Some(Uuid::from_bytes([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99,
        ]));
        task.arc_phase = Some("implement".into());

        let arc_info = ArcInfo {
            arc_title: "Test Arc".into(),
            current_phase: "implement".into(),
            phase_position: "phase 2 of 3".into(),
        };

        let bundle = working(&task, &[], &[], 10, Some(arc_info));
        assert_eq!(bundle.shape, "working");
        let arc = bundle.arc_info.as_ref().unwrap();
        assert_eq!(arc.arc_title, "Test Arc");
        assert_eq!(arc.current_phase, "implement");
        assert_eq!(arc.phase_position, "phase 2 of 3");
    }

    #[test]
    fn planning_shape_includes_prior_phase_context() {
        let mut task = make_task(1, "proj", 1);
        task.arc_id = Some(Uuid::from_bytes([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99,
        ]));
        task.arc_phase = Some("implement".into());

        let arc_info = ArcInfo {
            arc_title: "Test Arc".into(),
            current_phase: "implement".into(),
            phase_position: "phase 2 of 3".into(),
        };

        let mut prior_task = make_task(2, "proj", 2);
        prior_task.arc_phase = Some("design".into());
        prior_task.decisions = r#"[{"text":"use REST not gRPC"}]"#.into();
        prior_task.execution_record = Some("designed the API schema".into());

        let bundle = planning(&task, &[], &[], 10, Some(arc_info), &[prior_task]);
        assert_eq!(bundle.shape, "planning");
        assert!(bundle.arc_info.is_some());
        assert_eq!(bundle.prior_phase_decisions.len(), 1);
        assert_eq!(bundle.prior_phase_decisions[0]["text"], "use REST not gRPC");
        assert_eq!(bundle.prior_phase_execution_records.len(), 1);
        assert_eq!(bundle.prior_phase_execution_records[0]["task"], "proj/2");
        assert_eq!(bundle.prior_phase_execution_records[0]["phase"], "design");
        assert_eq!(
            bundle.prior_phase_execution_records[0]["record"],
            "designed the API schema"
        );
    }

    #[test]
    fn agent_shape_includes_arc_position_and_prior_context() {
        let mut task = make_task(1, "proj", 1);
        task.arc_id = Some(Uuid::from_bytes([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99,
        ]));
        task.arc_phase = Some("implement".into());

        let arc_info = ArcInfo {
            arc_title: "Test Arc".into(),
            current_phase: "implement".into(),
            phase_position: "phase 2 of 3".into(),
        };

        let mut prior_task = make_task(2, "proj", 2);
        prior_task.arc_phase = Some("design".into());
        prior_task.decisions = r#"[{"text":"use REST"}]"#.into();
        prior_task.execution_record = Some("API schema done".into());

        let bundle = agent(&task, Some(arc_info.clone()), &[prior_task]);
        assert_eq!(bundle.shape, "agent");
        let arc = bundle.arc_info.as_ref().unwrap();
        assert_eq!(arc.arc_title, "Test Arc");
        assert_eq!(arc.phase_position, "phase 2 of 3");
        assert_eq!(bundle.acceptance_criteria.len(), 1);
        assert_eq!(bundle.prior_phase_decisions.len(), 1);
        assert_eq!(bundle.prior_phase_execution_records.len(), 1);
    }

    #[test]
    fn cross_arc_blockers_from_edges() {
        let mut arc_task = make_task(1, "proj", 1);
        arc_task.arc_id = Some(Uuid::from_bytes([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99,
        ]));
        arc_task.arc_phase = Some("implement".into());

        let mut external_task = make_task(2, "proj", 2);
        external_task.status = TaskStatus::InProgress;

        let edges = vec![make_edge(1, 2, "depends_on")];

        let blockers = find_cross_arc_blockers(&[arc_task], &[external_task], &edges);
        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0]["blocker_task"], "proj/2");
        assert_eq!(blockers[0]["blocker_status"], "in-progress");
        assert_eq!(blockers[0]["blocked_task"], "proj/1");
    }

    #[test]
    fn cross_arc_blockers_ignores_done_externals() {
        let mut arc_task = make_task(1, "proj", 1);
        arc_task.arc_id = Some(Uuid::from_bytes([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99,
        ]));

        let mut external_task = make_task(2, "proj", 2);
        external_task.status = TaskStatus::Done;

        let edges = vec![make_edge(1, 2, "depends_on")];
        let blockers = find_cross_arc_blockers(&[arc_task], &[external_task], &edges);
        assert!(blockers.is_empty());
    }

    #[test]
    fn arc_summary_returns_phases_with_task_counts() {
        let phases = vec![
            serde_json::json!({"name": "design", "status": "completed"}),
            serde_json::json!({"name": "implement", "status": "active"}),
            serde_json::json!({"name": "review", "status": "pending"}),
        ];

        let mut t1 = make_task(1, "proj", 1);
        t1.arc_phase = Some("design".into());
        t1.status = TaskStatus::Done;

        let mut t2 = make_task(2, "proj", 2);
        t2.arc_phase = Some("implement".into());
        t2.status = TaskStatus::InProgress;

        let mut t3 = make_task(3, "proj", 3);
        t3.arc_phase = Some("implement".into());
        t3.status = TaskStatus::Done;

        let bundle = arc_summary("proj/~1", "Test Arc", "active", &phases, &[t1, t2, t3], &[]);
        assert_eq!(bundle.shape, "arc_summary");
        assert_eq!(bundle.human_id, "proj/~1");
        assert_eq!(bundle.title, "Test Arc");
        assert_eq!(bundle.status, "active");
        assert_eq!(bundle.phases.len(), 3);

        assert_eq!(bundle.phases[0]["name"], "design");
        assert_eq!(bundle.phases[0]["done_count"], 1);
        assert_eq!(bundle.phases[0]["total_count"], 1);

        assert_eq!(bundle.phases[1]["name"], "implement");
        assert_eq!(bundle.phases[1]["done_count"], 1);
        assert_eq!(bundle.phases[1]["total_count"], 2);

        assert_eq!(bundle.phases[2]["name"], "review");
        assert_eq!(bundle.phases[2]["total_count"], 0);
    }

    #[test]
    fn agent_shape_minimal_without_arc() {
        let task = make_task(1, "proj", 1);
        let bundle = agent(&task, None, &[]);
        assert_eq!(bundle.shape, "agent");
        assert!(bundle.arc_info.is_none());
        assert!(bundle.prior_phase_decisions.is_empty());
        assert!(bundle.prior_phase_execution_records.is_empty());
    }

    #[test]
    fn planning_shape_skips_tasks_without_execution_record() {
        let task = make_task(1, "proj", 1);

        let mut prior_task = make_task(2, "proj", 2);
        prior_task.arc_phase = Some("design".into());
        prior_task.execution_record = None;
        prior_task.decisions = "[]".into();

        let bundle = planning(&task, &[], &[], 10, None, &[prior_task]);
        assert_eq!(bundle.prior_phase_decisions.len(), 0);
        assert_eq!(bundle.prior_phase_execution_records.len(), 0);
    }

    #[test]
    fn working_shape_no_arc_info_when_no_arc() {
        let task = make_task(1, "proj", 1);
        let bundle = working(&task, &[], &[], 10, None);
        assert!(bundle.arc_info.is_none());
    }

    #[test]
    fn neighbor_ids_deduplicates() {
        let id1 = Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        let edges = vec![
            make_edge(1, 2, "depends_on"),
            make_edge(1, 2, "relates_to"),
            make_edge(3, 1, "motivated_by"),
        ];
        let ids = neighbor_ids(id1, &edges);
        assert_eq!(ids.len(), 2);
    }
}
