use chrono::TimeZone;
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};

use crate::db::{ProjectRow, TaskRow};

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

pub fn format_tasks_list(tasks: &[TaskRow]) -> String {
    if tasks.is_empty() {
        return "No tasks found.".to_string();
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec!["ID", "Title", "Status", "Category"]);
    for t in tasks {
        let human_id = format!("{}/{}", t.project_slug, t.sequence_number);
        let category = t.category.as_deref().unwrap_or("-");
        table.add_row(vec![
            human_id.as_str(),
            t.title.as_str(),
            t.status.as_str(),
            category,
        ]);
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

pub fn format_task_detail(t: &TaskRow) -> String {
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

    let tags: Vec<String> = serde_json::from_str(&t.tags).unwrap_or_default();
    if !tags.is_empty() {
        out.push_str(&format!("Tags:     {}\n", tags.join(", ")));
    }

    out.push_str(&format!("Created:  {}\n", format_ts(t.created_at)));
    out.push_str(&format!("Updated:  {}\n", format_ts(t.updated_at)));

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

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

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
            acceptance_criteria: r#"[{"text":"it works","done":true},{"text":"it's fast","done":false}]"#.to_string(),
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
        let out = format_tasks_list(&tasks);
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
        let out = format_tasks_list(&[]);
        assert_eq!(out, "No tasks found.");
    }

    #[test]
    fn task_detail_shows_fields_and_criteria() {
        let t = sample_task();
        let out = format_task_detail(&t);
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
        let out = format_task_detail(&t);
        assert!(!out.contains("Acceptance Criteria:"));
        assert!(!out.contains("Decisions:"));
    }
}
