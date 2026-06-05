use yojana::context;
use yojana::db::{CreateTaskParams, Db, TaskQueryFilter, TaskUpdates};
use yojana::graph;

#[test]
fn full_flow_v0() {
    // 1. In-memory DB
    let db = Db::open_in_memory().unwrap();

    // 2. Create project
    let project = db
        .create_project("yojana", "Yojana task graph server", "", None)
        .unwrap();
    assert_eq!(project.slug, "yojana");

    // 3. Create three tasks: A (no deps), B (depends_on A), C (depends_on B)
    let task_a = db
        .create_task(CreateTaskParams {
            project_id: project.id,
            project_slug: project.slug.clone(),
            title: "Task A".into(),
            description: "First task".into(),
            category: Some("enhancement".into()),
            status: None,
            slice_type: Some("AFK".into()),
            acceptance_criteria: r#"[{"text":"A works","done":false}]"#.into(),
            decisions: "[]".into(),
            context_refs: "[]".into(),
            files: "[]".into(),
            tags: r#"["infra"]"#.into(),
            implementation_plan: None,
            execution_record: None,
            reproduction: None,
            root_cause: None,
            arc_id: None,
            arc_phase: None,
        })
        .unwrap();

    let task_b = db
        .create_task(CreateTaskParams {
            project_id: project.id,
            project_slug: project.slug.clone(),
            title: "Task B".into(),
            description: "Second task".into(),
            category: Some("enhancement".into()),
            status: None,
            slice_type: Some("AFK".into()),
            acceptance_criteria: r#"[{"text":"B works","done":false}]"#.into(),
            decisions: r#"[{"text":"use approach X"}]"#.into(),
            context_refs: r#"[{"type":"git:commit","value":"abc123"}]"#.into(),
            files: "[]".into(),
            tags: "[]".into(),
            implementation_plan: None,
            execution_record: None,
            reproduction: None,
            root_cause: None,
            arc_id: None,
            arc_phase: None,
        })
        .unwrap();

    let task_c = db
        .create_task(CreateTaskParams {
            project_id: project.id,
            project_slug: project.slug.clone(),
            title: "Task C".into(),
            description: "Third task".into(),
            category: Some("enhancement".into()),
            status: None,
            slice_type: Some("HITL".into()),
            acceptance_criteria: "[]".into(),
            decisions: "[]".into(),
            context_refs: "[]".into(),
            files: "[]".into(),
            tags: "[]".into(),
            implementation_plan: None,
            execution_record: None,
            reproduction: None,
            root_cause: None,
            arc_id: None,
            arc_phase: None,
        })
        .unwrap();

    assert_eq!(task_a.sequence_number, 1);
    assert_eq!(task_b.sequence_number, 2);
    assert_eq!(task_c.sequence_number, 3);

    db.create_edge(task_b.id, task_a.id, "depends_on", None)
        .unwrap();
    db.create_edge(task_c.id, task_b.id, "depends_on", None)
        .unwrap();

    // 4. Ready shows only A (it has no deps; B and C are blocked)
    let deps = db.list_depends_on_with_status().unwrap();
    assert!(graph::is_ready(task_a.id, &deps));
    assert!(!graph::is_ready(task_b.id, &deps));
    assert!(!graph::is_ready(task_c.id, &deps));

    // 5. Transition A through the pipeline
    let id_a = task_a.id.to_string();
    db.update_task(
        &id_a,
        TaskUpdates {
            status: Some("ready-for-agent".into()),
            ..Default::default()
        },
    )
    .unwrap();
    db.update_task(
        &id_a,
        TaskUpdates {
            status: Some("in-progress".into()),
            ..Default::default()
        },
    )
    .unwrap();
    db.update_task(
        &id_a,
        TaskUpdates {
            status: Some("done".into()),
            ..Default::default()
        },
    )
    .unwrap();

    // 6. Now B should be ready (A is done), C still blocked
    let deps = db.list_depends_on_with_status().unwrap();
    assert!(graph::is_ready(task_b.id, &deps));
    assert!(!graph::is_ready(task_c.id, &deps));

    // 7. Summary context for B — edge counts and status
    let b_edges = db.list_edges_for_task(&task_b.id).unwrap();
    let summary_b = context::summary(&task_b, &b_edges);
    assert_eq!(summary_b.human_id, "yojana/2");
    assert_eq!(summary_b.title, "Task B");
    assert_eq!(summary_b.status, "needs-triage");
    assert_eq!(summary_b.edge_counts.get("depends_on_out"), Some(&1));

    // 8. Working context for C — shows B and A as neighbors (C depends_on B via edge)
    let c_edges = db.list_edges_for_task(&task_c.id).unwrap();
    let nids = context::neighbor_ids(task_c.id, &c_edges);
    assert_eq!(nids.len(), 1); // C only directly connects to B

    let mut neighbors_with_edges = Vec::new();
    for nid in &nids {
        let ntask = db.get_task(&nid.to_string()).unwrap().unwrap();
        let nedges = db.list_edges_for_task(&ntask.id).unwrap();
        neighbors_with_edges.push((ntask, nedges));
    }

    let c_messages = db.get_conversation_messages(&task_c.id).unwrap();
    let working_c = context::working(&task_c, &neighbors_with_edges, &c_messages, 10, None);
    assert_eq!(working_c.human_id, "yojana/3");
    assert_eq!(working_c.neighbors.len(), 1);
    assert_eq!(working_c.neighbors[0].human_id, "yojana/2");

    // 9. Add conversation message to B
    db.append_conversation_message(&task_b.id, "Starting implementation", Some("agent"))
        .unwrap();

    // 10. Working context for B — conversation appears
    let b_edges = db.list_edges_for_task(&task_b.id).unwrap();
    let b_nids = context::neighbor_ids(task_b.id, &b_edges);
    let mut b_neighbors = Vec::new();
    for nid in &b_nids {
        let ntask = db.get_task(&nid.to_string()).unwrap().unwrap();
        let nedges = db.list_edges_for_task(&ntask.id).unwrap();
        b_neighbors.push((ntask, nedges));
    }
    let b_messages = db.get_conversation_messages(&task_b.id).unwrap();
    let working_b = context::working(&task_b, &b_neighbors, &b_messages, 10, None);
    assert_eq!(working_b.recent_messages.len(), 1);
    assert_eq!(
        working_b.recent_messages[0]["text"],
        "Starting implementation"
    );
    assert_eq!(working_b.recent_messages[0]["author"], "agent");
    assert_eq!(working_b.context_refs.len(), 1);
    assert_eq!(working_b.context_refs[0].ref_type, yojana::tools::context_ref::RefType::GitCommit);

    // 11. Query by status=done — only A
    let done_tasks = db
        .list_tasks(&TaskQueryFilter {
            status: Some("done".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(done_tasks.len(), 1);
    assert_eq!(done_tasks[0].title, "Task A");

    // 12. Query across all projects — works with one project
    let all_tasks = db.list_tasks(&TaskQueryFilter::default()).unwrap();
    assert_eq!(all_tasks.len(), 3);
}
