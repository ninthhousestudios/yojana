use std::io::Write;
use std::process::{Command, Stdio};

use yojana::config::Config;
use yojana::db::Db;

fn unique_db() -> std::path::PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("yojana-todo-cli-{pid}-{nanos}.db"))
}

fn seed(db_path: &std::path::Path) -> Db {
    unsafe {
        std::env::set_var("YOJANA_DB_PATH", db_path);
    }
    let config = Config::from_env();
    Db::open(&config).unwrap()
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_yojana"))
}

#[test]
fn todo_creates_needs_triage_task_and_prints_slug() {
    let db_path = unique_db();
    {
        let db = seed(&db_path);
        db.create_project("demo", "Demo", "", None, "test").unwrap();
    }

    let out = cli()
        .args(["todo", "demo", "first todo"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), "demo/1");

    let db = seed(&db_path);
    let task = db.get_task("demo/1").unwrap().unwrap();
    assert_eq!(task.title, "first todo");
    assert_eq!(task.status, "needs-triage");
    assert_eq!(task.description, "");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn todo_with_message_flag_populates_description() {
    let db_path = unique_db();
    {
        let db = seed(&db_path);
        db.create_project("demo2", "Demo2", "", None, "test")
            .unwrap();
    }

    let out = cli()
        .args(["todo", "demo2", "title here", "-m", "body content"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "demo2/1");

    let db = seed(&db_path);
    let task = db.get_task("demo2/1").unwrap().unwrap();
    assert_eq!(task.description, "body content");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn todo_reads_stdin_when_no_message_and_piped() {
    let db_path = unique_db();
    {
        let db = seed(&db_path);
        db.create_project("demo3", "Demo3", "", None, "test")
            .unwrap();
    }

    let mut child = cli()
        .args(["todo", "demo3", "from stdin"])
        .env("YOJANA_DB_PATH", &db_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"piped body\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let db = seed(&db_path);
    let task = db.get_task("demo3/1").unwrap().unwrap();
    assert_eq!(task.description, "piped body");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn todo_unknown_project_exits_nonzero() {
    let db_path = unique_db();
    {
        let _ = seed(&db_path);
    }

    let out = cli()
        .args(["todo", "no-such-project", "x"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no-such-project"), "stderr was: {stderr}");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn todo_works_with_nested_project_slug() {
    let db_path = unique_db();
    {
        let db = seed(&db_path);
        let parent = db
            .create_project("demo4", "Demo4", "", None, "test")
            .unwrap();
        db.create_project("demo4/sub", "Sub", "", Some(parent.id), "test")
            .unwrap();
    }

    let out = cli()
        .args(["todo", "demo4/sub", "nested todo"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "demo4/sub/1");

    let _ = std::fs::remove_file(&db_path);
}

fn create_in_progress_task(db: &Db, project_slug: &str, title: &str) -> String {
    let proj = db.get_project(None, Some(project_slug)).unwrap().unwrap();
    let row = db
        .create_task(
            yojana::db::CreateTaskParams {
                project_id: proj.id,
                project_slug: proj.slug.clone(),
                title: title.to_string(),
                description: String::new(),
                category: None,
                status: Some("in-progress".to_string()),
                slice_type: None,
                acceptance_criteria: "[]".to_string(),
                decisions: "[]".to_string(),
                context_refs: "[]".to_string(),
                files: "[]".to_string(),
                tags: "[]".to_string(),
                implementation_plan: None,
                execution_record: None,
                reproduction: None,
                root_cause: None,
                arc_id: None,
                arc_phase: None,
            },
            "test",
        )
        .unwrap();
    format!("{}/{}", row.project_slug, row.sequence_number)
}

#[test]
fn done_with_message_writes_tagged_comment() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("dm", "DM", "", None, "test").unwrap();
        create_in_progress_task(&db, "dm", "some feature")
    };

    let out = cli()
        .args(["done", &human_id, "-m", "shipped the widget"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("→ done"), "stdout: {stdout}");

    let db = seed(&db_path);
    let task = db.get_task(&human_id).unwrap().unwrap();
    assert_eq!(task.status, "done");
    let msgs = db.get_conversation_messages(&task.id).unwrap();
    assert_eq!(msgs.len(), 1);
    let text = msgs[0]["text"].as_str().unwrap();
    assert_eq!(text, "[close:done] shipped the widget");
    assert_eq!(msgs[0]["author"].as_str().unwrap(), "josh");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn done_without_message_no_comment() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("dn", "DN", "", None, "test").unwrap();
        create_in_progress_task(&db, "dn", "quiet close")
    };

    let out = cli()
        .args(["done", &human_id])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(out.status.success());

    let db = seed(&db_path);
    let task = db.get_task(&human_id).unwrap().unwrap();
    assert_eq!(task.status, "done");
    let msgs = db.get_conversation_messages(&task.id).unwrap();
    assert!(msgs.is_empty());

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn wontfix_misfiled() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("wm", "WM", "", None, "test").unwrap();
        create_in_progress_task(&db, "wm", "wrong task")
    };

    let out = cli()
        .args(["wontfix", &human_id, "--reason", "misfiled"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("→ wontfix (misfiled)"), "stdout: {stdout}");

    let db = seed(&db_path);
    let task = db.get_task(&human_id).unwrap().unwrap();
    assert_eq!(task.status, "wontfix");
    assert!(task.completed_at.is_some());
    let msgs = db.get_conversation_messages(&task.id).unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(
        msgs[0]["text"].as_str().unwrap(),
        "[close:wontfix:misfiled]"
    );

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn wontfix_descoped_with_note() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("wd", "WD", "", None, "test").unwrap();
        create_in_progress_task(&db, "wd", "deferred work")
    };

    let out = cli()
        .args([
            "wontfix",
            &human_id,
            "--reason",
            "descoped",
            "-m",
            "punted to v2",
        ])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(out.status.success());

    let db = seed(&db_path);
    let task = db.get_task(&human_id).unwrap().unwrap();
    assert_eq!(task.status, "wontfix");
    let msgs = db.get_conversation_messages(&task.id).unwrap();
    assert_eq!(
        msgs[0]["text"].as_str().unwrap(),
        "[close:wontfix:descoped] punted to v2"
    );

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn wontfix_superseded_creates_edge() {
    let db_path = unique_db();
    let (old_id, new_id) = {
        let db = seed(&db_path);
        db.create_project("ws", "WS", "", None, "test").unwrap();
        let old = create_in_progress_task(&db, "ws", "old approach");
        let new_task = create_in_progress_task(&db, "ws", "new approach");
        (old, new_task)
    };

    let out = cli()
        .args([
            "wontfix",
            &old_id,
            "--reason",
            &format!("superseded={new_id}"),
        ])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let db = seed(&db_path);
    let old_task = db.get_task(&old_id).unwrap().unwrap();
    assert_eq!(old_task.status, "wontfix");

    let msgs = db.get_conversation_messages(&old_task.id).unwrap();
    let text = msgs[0]["text"].as_str().unwrap();
    assert!(
        text.starts_with("[close:wontfix:superseded] superseded by ws/2"),
        "comment was: {text}"
    );

    let edges = db.list_edges_for_task(&old_task.id).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].edge_type, "supersedes");
    let new_task = db.get_task(&new_id).unwrap().unwrap();
    assert_eq!(edges[0].source_task_id, new_task.id);
    assert_eq!(edges[0].target_task_id, old_task.id);

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn wontfix_superseded_nonexistent_target() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("wn", "WN", "", None, "test").unwrap();
        create_in_progress_task(&db, "wn", "to close")
    };

    let out = cli()
        .args(["wontfix", &human_id, "--reason", "superseded=wn/999"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("wn/999"), "stderr: {stderr}");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn wontfix_invalid_reason() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("wi", "WI", "", None, "test").unwrap();
        create_in_progress_task(&db, "wi", "test task")
    };

    let out = cli()
        .args(["wontfix", &human_id, "--reason", "banana"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown reason"), "stderr: {stderr}");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn wontfix_missing_reason() {
    let out = cli().args(["wontfix", "any/1"]).output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--reason"), "stderr: {stderr}");
}

#[test]
fn wontfix_from_ready_for_agent_uses_force() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("wf", "WF", "", None, "test").unwrap();
        let proj = db.get_project(None, Some("wf")).unwrap().unwrap();
        let row = db
            .create_task(
                yojana::db::CreateTaskParams {
                    project_id: proj.id,
                    project_slug: proj.slug.clone(),
                    title: "agent task".to_string(),
                    description: String::new(),
                    category: None,
                    status: Some("ready-for-agent".to_string()),
                    slice_type: None,
                    acceptance_criteria: "[]".to_string(),
                    decisions: "[]".to_string(),
                    context_refs: "[]".to_string(),
                    files: "[]".to_string(),
                    tags: "[]".to_string(),
                    implementation_plan: None,
                    execution_record: None,
                    reproduction: None,
                    root_cause: None,
                    arc_id: None,
                    arc_phase: None,
                },
                "test",
            )
            .unwrap();
        format!("{}/{}", row.project_slug, row.sequence_number)
    };

    let out = cli()
        .args(["wontfix", &human_id, "--reason", "obsolete"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let db = seed(&db_path);
    let task = db.get_task(&human_id).unwrap().unwrap();
    assert_eq!(task.status, "wontfix");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn wontfix_superseded_bad_target_leaves_task_unchanged() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("wb", "WB", "", None, "test").unwrap();
        create_in_progress_task(&db, "wb", "should stay open")
    };

    let out = cli()
        .args(["wontfix", &human_id, "--reason", "superseded=wb/999"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(!out.status.success());

    let db = seed(&db_path);
    let task = db.get_task(&human_id).unwrap().unwrap();
    assert_eq!(
        task.status, "in-progress",
        "task should not have been mutated"
    );
    let msgs = db.get_conversation_messages(&task.id).unwrap();
    assert!(msgs.is_empty(), "no comment should have been written");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn done_bug_task_no_message_non_tty_skips_prompt() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("db", "DB", "", None, "test").unwrap();
        let proj = db.get_project(None, Some("db")).unwrap().unwrap();
        let row = db
            .create_task(
                yojana::db::CreateTaskParams {
                    project_id: proj.id,
                    project_slug: proj.slug.clone(),
                    title: "a bug".to_string(),
                    description: String::new(),
                    category: Some("bug".to_string()),
                    status: Some("in-progress".to_string()),
                    slice_type: None,
                    acceptance_criteria: "[]".to_string(),
                    decisions: "[]".to_string(),
                    context_refs: "[]".to_string(),
                    files: "[]".to_string(),
                    tags: "[]".to_string(),
                    implementation_plan: None,
                    execution_record: None,
                    reproduction: None,
                    root_cause: None,
                    arc_id: None,
                    arc_phase: None,
                },
                "test",
            )
            .unwrap();
        format!("{}/{}", row.project_slug, row.sequence_number)
    };

    // Non-TTY (piped) without -m: prompt is skipped, no comment written
    let out = cli()
        .args(["done", &human_id])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let db = seed(&db_path);
    let task = db.get_task(&human_id).unwrap().unwrap();
    assert_eq!(task.status, "done");
    let msgs = db.get_conversation_messages(&task.id).unwrap();
    assert!(
        msgs.is_empty(),
        "non-TTY bug close without -m should skip prompt"
    );

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn done_bug_task_with_message_writes_comment() {
    let db_path = unique_db();
    let human_id = {
        let db = seed(&db_path);
        db.create_project("ds", "DS", "", None, "test").unwrap();
        let proj = db.get_project(None, Some("ds")).unwrap().unwrap();
        let row = db
            .create_task(
                yojana::db::CreateTaskParams {
                    project_id: proj.id,
                    project_slug: proj.slug.clone(),
                    title: "another bug".to_string(),
                    description: String::new(),
                    category: Some("bug".to_string()),
                    status: Some("in-progress".to_string()),
                    slice_type: None,
                    acceptance_criteria: "[]".to_string(),
                    decisions: "[]".to_string(),
                    context_refs: "[]".to_string(),
                    files: "[]".to_string(),
                    tags: "[]".to_string(),
                    implementation_plan: None,
                    execution_record: None,
                    reproduction: None,
                    root_cause: None,
                    arc_id: None,
                    arc_phase: None,
                },
                "test",
            )
            .unwrap();
        format!("{}/{}", row.project_slug, row.sequence_number)
    };

    let out = cli()
        .args(["done", &human_id, "-m", "null deref on empty list"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(out.status.success());

    let db = seed(&db_path);
    let task = db.get_task(&human_id).unwrap().unwrap();
    assert_eq!(task.status, "done");
    let msgs = db.get_conversation_messages(&task.id).unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(
        msgs[0]["text"].as_str().unwrap(),
        "[close:done] null deref on empty list"
    );

    let _ = std::fs::remove_file(&db_path);
}
