use yojana::context;
use yojana::db::{CreateTaskParams, Db, TaskUpdates};

fn make_task(
    db: &Db,
    project_id: uuid::Uuid,
    title: &str,
    desc: &str,
    commits: &[&str],
    deps: &[&str],
) -> yojana::db::TaskRow {
    let ac: Vec<serde_json::Value> = deps
        .iter()
        .map(|d| serde_json::json!({"text": format!("depends on {d}"), "done": false}))
        .collect();
    let refs: Vec<serde_json::Value> = commits
        .iter()
        .map(|c| serde_json::json!({"type": "git:commit", "value": c}))
        .collect();

    db.create_task(
        CreateTaskParams {
            project_id,
            project_slug: "yojana".into(),
            title: title.into(),
            description: desc.into(),
            category: Some("enhancement".into()),
            status: None,
            slice_type: Some("AFK".into()),
            acceptance_criteria: serde_json::to_string(&ac).unwrap(),
            decisions: "[]".into(),
            context_refs: serde_json::to_string(&refs).unwrap(),
            files: "[]".into(),
            tags: "[]".into(),
            implementation_plan: None,
            execution_record: None,
            reproduction: None,
            root_cause: None,
            arc_id: None,
            arc_phase: None,
        },
        "test",
    )
    .unwrap()
}

#[test]
fn bootstrap_v0_review() {
    let db = Db::open_in_memory().unwrap();
    let project = db
        .create_project(
            "yojana",
            "Yojana task graph server",
            "Local-first task graph for the manas ecosystem",
            None,
            "test",
        )
        .unwrap();
    let pid = project.id;

    // Slices 01-02: project + task CRUD
    let s01 = make_task(
        &db,
        pid,
        "Slices 01-02: Project + Task CRUD with MCP server",
        "HTTP/stdio MCP server with SQLite storage. Project CRUD with slug uniqueness, status, history. Task CRUD with per-project sequence numbers, JSON columns, context_refs validation, partial updates.",
        &["2ec4b69"],
        &[],
    );

    // Slice 03-04: state machine + edges
    let s03 = make_task(
        &db,
        pid,
        "Slices 03-04: State machine + edges with cycle detection",
        "Pure state machine module validating triage label transitions. Edge CRUD with 5 typed relationships. DFS cycle detection on depends_on edges.",
        &["bd03edd"],
        &[],
    );

    // Slice 05: query + ready
    let s05 = make_task(
        &db,
        pid,
        "Slice 05: Query filtering and ready detection",
        "yojana_query with project/status/category/slice_type/tag filters. yojana_ready shortcut. Graph-based ready/blocked computation from dependency edges.",
        &["64dee68"],
        &[],
    );

    // Slice 06: context shapes
    let s06 = make_task(
        &db,
        pid,
        "Slice 06: Context shapes (summary + working) and conversations",
        "Pure context assembler module. Summary shape: title, status, edge counts, last history. Working shape: AC, decisions, 1-hop neighbors, conversation messages, context_refs. task_conversations table. Comment action on yojana_task.",
        &["7c9671b"],
        &[],
    );

    // Slice 07: e2e test
    let s07 = make_task(
        &db,
        pid,
        "Slice 07: End-to-end integration test",
        "Full-flow test: project → tasks → edges → ready detection → state transitions → context shapes → conversations → query filtering. 12-step validation of v0 stack.",
        &["e796b8e"],
        &[],
    );

    // Slice 08: mp-skills adapter
    let s08 = make_task(
        &db,
        pid,
        "Slice 08: mp-skills issue tracker adapter",
        "Documentation mapping mp-skills operations to yojana MCP tool calls. Spike/experiment conventions. Updated issue-tracker.md to reference yojana as active backend.",
        &["d5bfee0"],
        &[],
    );

    // Wire dependency chain
    db.create_edge(s03.id, s01.id, "depends_on", None).unwrap();
    db.create_edge(s05.id, s03.id, "depends_on", None).unwrap();
    db.create_edge(s06.id, s05.id, "depends_on", None).unwrap();
    db.create_edge(s07.id, s06.id, "depends_on", None).unwrap();
    db.create_edge(s08.id, s06.id, "depends_on", None).unwrap();

    // Mark all done
    for task in [&s01, &s03, &s05, &s06, &s07, &s08] {
        let id = task.id.to_string();
        db.update_task(
            &id,
            TaskUpdates {
                status: Some("ready-for-agent".into()),
                ..Default::default()
            },
            "test",
        )
        .unwrap();
        db.update_task(
            &id,
            TaskUpdates {
                status: Some("in-progress".into()),
                ..Default::default()
            },
            "test",
        )
        .unwrap();
        db.update_task(
            &id,
            TaskUpdates {
                status: Some("done".into()),
                ..Default::default()
            },
            "test",
        )
        .unwrap();
    }

    // Fetch review context for each slice and verify structure
    for task_id in [s01.id, s03.id, s05.id, s06.id, s07.id, s08.id] {
        let task = db.get_task(&task_id.to_string()).unwrap().unwrap();
        let edges = db.list_edges_for_task(&task.id).unwrap();
        let nids = context::neighbor_ids(task.id, &edges);

        let mut neighbors_with_edges = Vec::new();
        for nid in &nids {
            if let Some(ntask) = db.get_task(&nid.to_string()).unwrap() {
                let nedges = db.list_edges_for_task(&ntask.id).unwrap();
                neighbors_with_edges.push((ntask, nedges));
            }
        }

        let bundle = context::review(&task, &neighbors_with_edges);
        assert_eq!(bundle.shape, "review");
        assert_eq!(bundle.status, "done");
        assert!(
            !bundle.git_refs.is_empty(),
            "task {} should have git refs",
            bundle.human_id
        );
        assert!(!bundle.description.is_empty());
    }

    // Verify slice 06 review has neighbor context (depends on s05, depended by s07 + s08)
    let s06_task = db.get_task(&s06.id.to_string()).unwrap().unwrap();
    let s06_edges = db.list_edges_for_task(&s06.id).unwrap();
    let s06_nids = context::neighbor_ids(s06.id, &s06_edges);
    let mut s06_neighbors = Vec::new();
    for nid in &s06_nids {
        let ntask = db.get_task(&nid.to_string()).unwrap().unwrap();
        let nedges = db.list_edges_for_task(&ntask.id).unwrap();
        s06_neighbors.push((ntask, nedges));
    }
    let s06_review = context::review(&s06_task, &s06_neighbors);
    assert_eq!(s06_review.git_refs.len(), 1);
    assert_eq!(s06_review.git_refs[0].value, "7c9671b");
    assert_eq!(s06_review.neighbors.len(), 3); // s05, s07, s08

    // Print one review bundle to show it works
    let output = serde_json::to_string_pretty(&s06_review).unwrap();
    assert!(output.contains("review"));
    assert!(output.contains("7c9671b"));
}
