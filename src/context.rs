use std::collections::HashMap;

use serde::Serialize;
use uuid::Uuid;

use crate::db::{EdgeRow, HistoryEntry, TaskRow};

pub const VALID_SHAPES: &[&str] = &["summary", "working", "review"];

#[derive(Debug, Serialize)]
pub struct SummaryBundle {
    pub shape: &'static str,
    pub human_id: String,
    pub title: String,
    pub status: String,
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
    pub context_refs: Vec<serde_json::Value>,
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
) -> WorkingBundle {
    let neighbor_summaries: Vec<SummaryBundle> = neighbors_with_edges
        .iter()
        .map(|(t, e)| summary(t, e))
        .collect();

    let recent: Vec<serde_json::Value> = if messages.len() > max_messages {
        messages[messages.len() - max_messages..].to_vec()
    } else {
        messages.to_vec()
    };

    WorkingBundle {
        shape: "working",
        human_id: human_id(task),
        acceptance_criteria: json_array(&task.acceptance_criteria),
        decisions: json_array(&task.decisions),
        neighbors: neighbor_summaries,
        recent_messages: recent,
        context_refs: json_array(&task.context_refs),
    }
}

#[derive(Debug, Serialize)]
pub struct ReviewBundle {
    pub shape: &'static str,
    pub human_id: String,
    pub title: String,
    pub status: String,
    pub description: String,
    pub acceptance_criteria: Vec<serde_json::Value>,
    pub decisions: Vec<serde_json::Value>,
    pub implementation_plan: Option<String>,
    pub git_refs: Vec<serde_json::Value>,
    pub doc_refs: Vec<serde_json::Value>,
    pub other_refs: Vec<serde_json::Value>,
    pub neighbors: Vec<SummaryBundle>,
}

pub fn review(
    task: &TaskRow,
    neighbors_with_edges: &[(TaskRow, Vec<EdgeRow>)],
) -> ReviewBundle {
    let all_refs = json_array(&task.context_refs);
    let mut git_refs = Vec::new();
    let mut doc_refs = Vec::new();
    let mut other_refs = Vec::new();

    for r in &all_refs {
        match r.get("type").and_then(|t| t.as_str()) {
            Some(t) if t.starts_with("git:") => git_refs.push(r.clone()),
            Some(t) if t.starts_with("doc:") => doc_refs.push(r.clone()),
            _ => other_refs.push(r.clone()),
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
        acceptance_criteria: json_array(&task.acceptance_criteria),
        decisions: json_array(&task.decisions),
        implementation_plan: task.implementation_plan.clone(),
        git_refs,
        doc_refs,
        other_refs,
        neighbors: neighbor_summaries,
    }
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
            status: "ready-for-agent".into(),
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
        assert_eq!(bundle.status, "ready-for-agent");
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

    #[test]
    fn working_shape_with_neighbors() {
        let task = make_task(1, "proj", 1);
        let neighbor = make_task(2, "proj", 2);
        let neighbor_edges = vec![make_edge(1, 2, "depends_on")];

        let messages = vec![
            serde_json::json!({"ts": 1000, "text": "hello"}),
            serde_json::json!({"ts": 2000, "text": "world"}),
        ];

        let bundle = working(&task, &[(neighbor, neighbor_edges)], &messages, 10);
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

        let bundle = working(&task, &[], &messages, 5);
        assert_eq!(bundle.recent_messages.len(), 5);
        assert_eq!(bundle.recent_messages[0]["ts"], 15);
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
