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
        db.create_project("demo", "Demo", "", None).unwrap();
    }

    let out = cli()
        .args(["todo", "demo", "first todo"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
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
        db.create_project("demo2", "Demo2", "", None).unwrap();
    }

    let out = cli()
        .args(["todo", "demo2", "title here", "-m", "body content"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
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
        db.create_project("demo3", "Demo3", "", None).unwrap();
    }

    let mut child = cli()
        .args(["todo", "demo3", "from stdin"])
        .env("YOJANA_DB_PATH", &db_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"piped body\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

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
        let parent = db.create_project("demo4", "Demo4", "", None).unwrap();
        db.create_project("demo4/sub", "Sub", "", Some(parent.id)).unwrap();
    }

    let out = cli()
        .args(["todo", "demo4/sub", "nested todo"])
        .env("YOJANA_DB_PATH", &db_path)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "demo4/sub/1");

    let _ = std::fs::remove_file(&db_path);
}
