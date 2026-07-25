use chrono::TimeZone;
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};

use std::collections::HashMap;

use uuid::Uuid;

use crate::db::{ArcRow, ProjectRow, TaskRow};
use crate::graph::ForestNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    Outgoing,
    Incoming,
}

#[derive(Debug, Clone)]
pub struct EdgeDisplay {
    pub direction: EdgeDirection,
    pub edge_type: String,
    pub other_human_id: String,
    pub other_title: String,
}

struct PhaseInfo {
    name: String,
    status: String,
}

fn parse_phases(phases_json: &str) -> Vec<PhaseInfo> {
    let phases: Vec<serde_json::Value> = serde_json::from_str(phases_json).unwrap_or_default();
    phases
        .into_iter()
        .filter_map(|p| {
            let name = p.get("name")?.as_str()?.to_string();
            let status = p
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("pending")
                .to_string();
            Some(PhaseInfo { name, status })
        })
        .collect()
}

fn render_phase_dots(phases: &[PhaseInfo]) -> String {
    phases
        .iter()
        .map(|p| match p.status.as_str() {
            "completed" | "active" => '●',
            _ => '○',
        })
        .collect()
}

pub struct ArcDisplayInfo {
    pub id: Uuid,
    pub human_id: String,
    pub sequence_number: i64,
    pub title: String,
    pub status: String,
    phases: Vec<PhaseInfo>,
}

impl ArcDisplayInfo {
    pub fn from_row(row: &ArcRow) -> Self {
        let human_id = format!("{}/~{}", row.project_slug, row.sequence_number);
        let phases = parse_phases(&row.phases);
        Self {
            id: row.id,
            human_id,
            sequence_number: row.sequence_number,
            title: row.title.clone(),
            status: row.status.clone(),
            phases,
        }
    }

    pub fn dot_progress(&self) -> String {
        render_phase_dots(&self.phases)
    }

    pub fn phase_status(&self, phase_name: &str) -> &str {
        self.phases
            .iter()
            .find(|p| p.name == phase_name)
            .map(|p| p.status.as_str())
            .unwrap_or("?")
    }
}

fn edge_label(edge_type: &str, direction: EdgeDirection) -> &'static str {
    use EdgeDirection::*;
    match (edge_type, direction) {
        ("depends_on", Outgoing) => "Blocked by",
        ("depends_on", Incoming) => "Blocks",
        ("relates_to", _) => "Relates to",
        ("supersedes", Outgoing) => "Supersedes",
        ("supersedes", Incoming) => "Superseded by",
        ("refines", Outgoing) => "Refines",
        ("refines", Incoming) => "Refined by",
        ("motivated_by", Outgoing) => "Motivated by",
        ("motivated_by", Incoming) => "Motivates",
        (_, Outgoing) => "Out",
        (_, Incoming) => "In",
    }
}

fn format_ts(millis: i64) -> String {
    chrono::Local
        .timestamp_millis_opt(millis)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| millis.to_string())
}

pub fn format_projects_list(projects: &[ProjectRow]) -> String {
    if projects.is_empty() {
        return "No projects found.".to_string();
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec!["Slug", "Title", "Status"]);
    for p in projects {
        table.add_row(vec![&p.slug, &p.title, &p.status]);
    }
    table.to_string()
}

pub fn format_tasks_list(tasks: &[TaskRow], arcs: &HashMap<Uuid, ArcDisplayInfo>) -> String {
    if tasks.is_empty() {
        return "No tasks found.".to_string();
    }
    let show_completed = tasks.iter().any(|t| t.completed_at.is_some());
    let show_arc = tasks.iter().any(|t| t.arc_id.is_some());
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    let mut header = vec!["ID", "Title", "Status", "Category"];
    if show_arc {
        header.push("Arc");
    }
    if show_completed {
        header.push("Completed");
    }
    table.set_header(header);
    for t in tasks {
        let human_id = format!("{}/{}", t.project_slug, t.sequence_number);
        let category = t.category.as_deref().unwrap_or("-");
        let mut row = vec![
            human_id.clone(),
            t.title.clone(),
            t.status.clone(),
            category.to_string(),
        ];
        if show_arc {
            let arc_col = match (&t.arc_id, &t.arc_phase) {
                (Some(aid), Some(phase)) => match arcs.get(aid) {
                    Some(info) => format!("~{} {}", info.sequence_number, phase),
                    None => format!("~? {}", phase),
                },
                _ => "-".to_string(),
            };
            row.push(arc_col);
        }
        if show_completed {
            row.push(
                t.completed_at
                    .map(format_ts)
                    .unwrap_or_else(|| "-".to_string()),
            );
        }
        table.add_row(row);
    }
    table.to_string()
}

pub fn format_project_detail(p: &ProjectRow) -> String {
    let mut out = String::new();
    out.push_str(&format!("Project: {}\n", p.slug));
    out.push_str(&format!("Title:   {}\n", p.title));
    out.push_str(&format!("Status:  {}\n", p.status));
    out.push_str(&format!("Created: {}\n", format_ts(p.created_at)));
    out.push_str(&format!("Updated: {}\n", format_ts(p.updated_at)));
    if !p.description.is_empty() {
        let desc = p.description.replace("\\n", "\n");
        out.push_str(&format!("\n{}\n", desc));
    }
    out
}

pub fn format_task_detail(
    t: &TaskRow,
    edges: &[EdgeDisplay],
    messages: &[serde_json::Value],
    arc: Option<&ArcDisplayInfo>,
) -> String {
    let mut out = String::new();
    let human_id = format!("{}/{}", t.project_slug, t.sequence_number);

    out.push_str(&format!("Task:     {}\n", human_id));
    out.push_str(&format!("Title:    {}\n", t.title));
    out.push_str(&format!("Status:   {}\n", t.status));
    if let Some(cat) = &t.category {
        out.push_str(&format!("Category: {}\n", cat));
    }
    if let Some(st) = &t.slice_type {
        out.push_str(&format!("Slice:    {}\n", st));
    }
    if let (Some(arc_info), Some(phase)) = (arc, &t.arc_phase) {
        out.push_str(&format!(
            "Arc:      {} — {}\n",
            arc_info.human_id, arc_info.title
        ));
        let phase_status = arc_info.phase_status(phase);
        out.push_str(&format!(
            "Phase:    {} ({})  {}\n",
            phase,
            phase_status,
            arc_info.dot_progress()
        ));
    }

    let tags: Vec<String> = serde_json::from_str(&t.tags).unwrap_or_default();
    if !tags.is_empty() {
        out.push_str(&format!("Tags:     {}\n", tags.join(", ")));
    }

    out.push_str(&format!("Created:  {}\n", format_ts(t.created_at)));
    out.push_str(&format!("Updated:  {}\n", format_ts(t.updated_at)));

    if !edges.is_empty() {
        out.push_str("\nRelationships:\n");
        let mut grouped: Vec<(&'static str, Vec<&EdgeDisplay>)> = Vec::new();
        for e in edges {
            let label = edge_label(&e.edge_type, e.direction);
            if let Some((_, v)) = grouped.iter_mut().find(|(l, _)| *l == label) {
                v.push(e);
            } else {
                grouped.push((label, vec![e]));
            }
        }
        let label_width = grouped.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
        for (label, items) in grouped {
            for (i, e) in items.iter().enumerate() {
                let prefix = if i == 0 {
                    format!(
                        "  {:width$}  ",
                        format!("{}:", label),
                        width = label_width + 1
                    )
                } else {
                    " ".repeat(label_width + 5)
                };
                out.push_str(&format!(
                    "{}{} — {}\n",
                    prefix, e.other_human_id, e.other_title
                ));
            }
        }
    }

    if !t.description.is_empty() {
        let desc = t.description.replace("\\n", "\n");
        out.push_str(&format!("\n{}\n", desc));
    }

    #[derive(serde::Deserialize)]
    struct Criterion {
        text: String,
        done: bool,
    }
    let criteria: Vec<Criterion> = serde_json::from_str(&t.acceptance_criteria).unwrap_or_default();
    if !criteria.is_empty() {
        out.push_str("\nAcceptance Criteria:\n");
        for c in &criteria {
            let mark = if c.done { "[x]" } else { "[ ]" };
            out.push_str(&format!("  {} {}\n", mark, c.text));
        }
    }

    let decisions: Vec<serde_json::Value> = serde_json::from_str(&t.decisions).unwrap_or_default();
    if !decisions.is_empty() {
        out.push_str("\nDecisions:\n");
        for d in &decisions {
            if let Some(s) = d.as_str() {
                out.push_str(&format!("  - {}\n", s));
            } else {
                out.push_str(&format!("  - {}\n", d));
            }
        }
    }

    if let Some(plan) = &t.implementation_plan {
        out.push_str(&format!("\nImplementation Plan:\n{}\n", plan));
    }

    if let Some(record) = &t.execution_record {
        out.push_str(&format!("\nExecution Record:\n{}\n", record));
    }

    if !messages.is_empty() {
        out.push_str("\nComments:\n");
        for m in messages {
            let author = m.get("author").and_then(|v| v.as_str()).unwrap_or("?");
            let text = m.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let ts = m.get("ts").and_then(|v| v.as_i64()).map(format_ts);
            if let Some(ts) = ts {
                out.push_str(&format!("  [{}] {}: {}\n", ts, author, text));
            } else {
                out.push_str(&format!("  {}: {}\n", author, text));
            }
        }
    }

    out
}

pub fn format_dependency_tree(
    roots: &[ForestNode],
    standalone: &[Uuid],
    tasks: &HashMap<Uuid, &TaskRow>,
) -> String {
    if roots.is_empty() && standalone.is_empty() {
        return "No tasks found.".to_string();
    }
    let mut out = String::new();
    if roots.is_empty() && !standalone.is_empty() {
        out.push_str("No dependency edges found.\n\n");
    }
    for root in roots {
        render_node(&mut out, root, tasks, "", true, true);
    }
    if !standalone.is_empty() {
        if !roots.is_empty() {
            out.push('\n');
        }
        out.push_str("Standalone (no dependency edges):\n");
        for id in standalone {
            if let Some(t) = tasks.get(id) {
                out.push_str(&format!(
                    "  {}/{}  [{}] {}\n",
                    t.project_slug, t.sequence_number, t.status, t.title
                ));
            }
        }
    }
    out
}

pub fn format_arc_tree(
    arcs: &[ArcDisplayInfo],
    arc_tasks: &HashMap<Uuid, Vec<&TaskRow>>,
    dep_roots: &[ForestNode],
    standalone: &[Uuid],
    tasks: &HashMap<Uuid, &TaskRow>,
) -> String {
    if arcs.is_empty() && dep_roots.is_empty() && standalone.is_empty() {
        return "No tasks found.".to_string();
    }
    let mut out = String::new();

    for arc in arcs {
        let tasks_in_arc = match arc_tasks.get(&arc.id) {
            Some(t) if !t.is_empty() => t,
            _ => continue,
        };
        let dots = arc.dot_progress();
        out.push_str(&format!(
            "Arc: {} — {}  [{}]  {}\n",
            arc.human_id, arc.title, arc.status, dots
        ));
        for phase in &arc.phases {
            let phase_tasks: Vec<&&TaskRow> = tasks_in_arc
                .iter()
                .filter(|t| t.arc_phase.as_deref() == Some(&phase.name))
                .collect();
            if phase_tasks.is_empty() {
                continue;
            }
            out.push_str(&format!("  {} ({}):\n", phase.name, phase.status));
            for t in phase_tasks {
                out.push_str(&format!(
                    "    {}/{}  [{}] {}\n",
                    t.project_slug, t.sequence_number, t.status, t.title
                ));
            }
        }
        out.push('\n');
    }

    for root in dep_roots {
        render_node(&mut out, root, tasks, "", true, true);
    }

    if !standalone.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("Standalone (no dependency edges):\n");
        for id in standalone {
            if let Some(t) = tasks.get(id) {
                out.push_str(&format!(
                    "  {}/{}  [{}] {}\n",
                    t.project_slug, t.sequence_number, t.status, t.title
                ));
            }
        }
    }

    if out.is_empty() {
        "No tasks found.".to_string()
    } else {
        out
    }
}

fn render_node(
    out: &mut String,
    node: &ForestNode,
    tasks: &HashMap<Uuid, &TaskRow>,
    prefix: &str,
    is_last: bool,
    is_root: bool,
) {
    let connector = if is_root {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };

    if !is_root {
        // no extra spacing for children
    } else if !out.is_empty() {
        out.push('\n');
    }

    match tasks.get(&node.task_id) {
        Some(t) => {
            if node.seen_before {
                out.push_str(&format!(
                    "{prefix}{connector}{}/{}  (see above)\n",
                    t.project_slug, t.sequence_number
                ));
                return;
            }
            out.push_str(&format!(
                "{prefix}{connector}{}/{}  [{}] {}\n",
                t.project_slug, t.sequence_number, t.status, t.title
            ));
        }
        None => return,
    }

    let child_prefix = if is_root {
        String::new()
    } else if is_last {
        format!("{prefix}    ")
    } else {
        format!("{prefix}│   ")
    };

    for (i, child) in node.children.iter().enumerate() {
        let child_is_last = i == node.children.len() - 1;
        render_node(out, child, tasks, &child_prefix, child_is_last, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_project() -> ProjectRow {
        ProjectRow {
            id: Uuid::nil(),
            slug: "myproj".to_string(),
            title: "My Project".to_string(),
            description: "A test project".to_string(),
            status: "active".to_string(),
            parent_id: None,
            history: "[]".to_string(),
            created_at: 1700000000000,
            updated_at: 1700000060000,
        }
    }

    fn sample_task() -> TaskRow {
        TaskRow {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            project_slug: "myproj".to_string(),
            sequence_number: 1,
            title: "Do the thing".to_string(),
            description: "Build it well".to_string(),
            category: Some("enhancement".to_string()),
            status: "in-progress".to_string(),
            slice_type: Some("AFK".to_string()),
            acceptance_criteria:
                r#"[{"text":"it works","done":true},{"text":"it's fast","done":false}]"#.to_string(),
            decisions: r#"["use Rust"]"#.to_string(),
            implementation_plan: None,
            execution_record: None,
            reproduction: None,
            root_cause: None,
            context_refs: "[]".to_string(),
            files: "[]".to_string(),
            tags: r#"["cli","ux"]"#.to_string(),
            history: "[]".to_string(),
            created_at: 1700000000000,
            updated_at: 1700000060000,
            completed_at: None,
            arc_id: None,
            arc_phase: None,
        }
    }

    #[test]
    fn projects_list_shows_headers_and_data() {
        let projects = vec![sample_project()];
        let out = format_projects_list(&projects);
        assert!(out.contains("Slug"));
        assert!(out.contains("Title"));
        assert!(out.contains("Status"));
        assert!(out.contains("myproj"));
        assert!(out.contains("My Project"));
        assert!(out.contains("active"));
    }

    #[test]
    fn projects_list_empty() {
        let out = format_projects_list(&[]);
        assert_eq!(out, "No projects found.");
    }

    #[test]
    fn project_detail_shows_all_fields() {
        let p = sample_project();
        let out = format_project_detail(&p);
        assert!(out.contains("Project: myproj"));
        assert!(out.contains("Title:   My Project"));
        assert!(out.contains("Status:  active"));
        assert!(out.contains("A test project"));
    }

    #[test]
    fn tasks_list_shows_headers_and_data() {
        let tasks = vec![sample_task()];
        let out = format_tasks_list(&tasks, &HashMap::new());
        assert!(out.contains("ID"));
        assert!(out.contains("Title"));
        assert!(out.contains("Status"));
        assert!(out.contains("Category"));
        assert!(out.contains("myproj/1"));
        assert!(out.contains("Do the thing"));
        assert!(out.contains("in-progress"));
        assert!(out.contains("enhancement"));
    }

    #[test]
    fn tasks_list_empty() {
        let out = format_tasks_list(&[], &HashMap::new());
        assert_eq!(out, "No tasks found.");
    }

    #[test]
    fn task_detail_shows_fields_and_criteria() {
        let t = sample_task();
        let out = format_task_detail(&t, &[], &[], None);
        assert!(out.contains("Task:     myproj/1"));
        assert!(out.contains("Title:    Do the thing"));
        assert!(out.contains("Status:   in-progress"));
        assert!(out.contains("Category: enhancement"));
        assert!(out.contains("Slice:    AFK"));
        assert!(out.contains("Tags:     cli, ux"));
        assert!(out.contains("Build it well"));
        assert!(out.contains("[x] it works"));
        assert!(out.contains("[ ] it's fast"));
        assert!(out.contains("- use Rust"));
    }

    #[test]
    fn task_detail_empty_criteria_and_decisions() {
        let mut t = sample_task();
        t.acceptance_criteria = "[]".to_string();
        t.decisions = "[]".to_string();
        let out = format_task_detail(&t, &[], &[], None);
        assert!(!out.contains("Acceptance Criteria:"));
        assert!(!out.contains("Decisions:"));
    }

    #[test]
    fn task_detail_omits_relationships_when_empty() {
        let t = sample_task();
        let out = format_task_detail(&t, &[], &[], None);
        assert!(!out.contains("Relationships:"));
    }

    #[test]
    fn task_detail_renders_edges_grouped_by_label() {
        let t = sample_task();
        let edges = vec![
            EdgeDisplay {
                direction: EdgeDirection::Outgoing,
                edge_type: "depends_on".into(),
                other_human_id: "myproj/2".into(),
                other_title: "Upstream task".into(),
            },
            EdgeDisplay {
                direction: EdgeDirection::Incoming,
                edge_type: "depends_on".into(),
                other_human_id: "myproj/3".into(),
                other_title: "Downstream task".into(),
            },
            EdgeDisplay {
                direction: EdgeDirection::Incoming,
                edge_type: "depends_on".into(),
                other_human_id: "myproj/4".into(),
                other_title: "Other downstream".into(),
            },
            EdgeDisplay {
                direction: EdgeDirection::Outgoing,
                edge_type: "relates_to".into(),
                other_human_id: "smriti/1".into(),
                other_title: "Cross-project".into(),
            },
        ];
        let out = format_task_detail(&t, &edges, &[], None);
        assert!(out.contains("Relationships:"));
        assert!(out.contains("Blocked by:"));
        assert!(out.contains("myproj/2 — Upstream task"));
        assert!(out.contains("Blocks:"));
        assert!(out.contains("myproj/3 — Downstream task"));
        assert!(out.contains("myproj/4 — Other downstream"));
        assert!(out.contains("Relates to:"));
        assert!(out.contains("smriti/1 — Cross-project"));
        // Single label heading per group: "Blocks:" appears once even with two items.
        assert_eq!(out.matches("Blocks:").count(), 1);
    }
}
