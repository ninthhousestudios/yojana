//! `yojana export` — regenerate the committed in-repo task snapshot from the DB.
//!
//! A read-only convenience layer (PRD yojana/51): resolves the repo binding,
//! queries the root project's subtree, serializes a deterministic manifest, and
//! writes it plus a `.gitattributes` seatbelt. SQLite stays the sole source of
//! truth; export never mutates DB state (PRD I1).
//!
//! This slice ships the manifest layer only; the full-records layer (PRD
//! I4/I11/I13) lands in a later slice.

mod binding;
mod manifest;
mod record;
mod writer;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use uuid::Uuid;

use crate::config::Config;
use crate::db::{Db, TaskQueryFilter, TaskRow};
use crate::tools::task::TaskOutput;
use writer::RecordFile;

/// Run `yojana export`, starting the config walk-up from `cwd`.
pub fn run(cwd: PathBuf) -> anyhow::Result<()> {
    let binding = binding::find_config(&cwd)?;
    let config = Config::from_env();
    let db = Db::open(&config)?;
    let project_ids = binding::resolve_project_ids(&db, &binding.config.project)?;

    let tasks = db.list_tasks(&export_filter(project_ids))?;

    let bytes = manifest::serialize_manifest(&tasks);
    writer::write_manifest(&binding.repo_root, &bytes)?;
    writer::ensure_gitattributes(&binding.repo_root)?;
    let task_count = tasks.len();

    // Optional full-record layer for terminal tasks (PRD I4). Reconcile runs
    // even for an empty batch so a task that just left terminal has its record
    // dropped (I11); records=false skips the layer entirely (story 18) and,
    // deliberately, leaves any existing records/ untouched.
    let records_note = if binding.config.records {
        let records = collect_record_files(&db, tasks)?;
        let expected: HashSet<String> = records.iter().map(|r| r.filename.clone()).collect();
        writer::write_records(&binding.repo_root, &records)?;
        writer::reconcile_records(&binding.repo_root, &expected)?;
        format!(", {} record(s) to .yojana/records/", records.len())
    } else {
        String::new()
    };

    println!(
        "wrote {} task(s) to {}{}",
        task_count,
        binding.repo_root.join(".yojana/manifest.jsonl").display(),
        records_note,
    );
    Ok(())
}

/// Build one record file per terminal task: the full task envelope
/// (`TaskOutput` with conversation messages) plus its incident edges, each
/// endpoint resolved to a human id. Split out of [`run`] so it is testable over
/// an in-memory DB without the env-backed `Db::open` path.
///
/// Edges can leave the exported subtree (a cross-project `depends_on`), so the
/// endpoint->human_id map seeds from the loaded subtree and falls back to a DB
/// lookup for any endpoint not already present.
fn collect_record_files(db: &Db, tasks: Vec<TaskRow>) -> anyhow::Result<Vec<RecordFile>> {
    // Seed the endpoint->human_id map from the whole subtree before consuming
    // the rows, so an intra-subtree edge resolves without a DB round-trip.
    let mut human: HashMap<Uuid, String> = tasks
        .iter()
        .map(|t| (t.id, format!("{}/{}", t.project_slug, t.sequence_number)))
        .collect();

    let mut out = Vec::new();
    for row in tasks.into_iter().filter(|t| t.status.is_terminal()) {
        let task_id = row.id;
        let filename = record::record_filename(&row.project_slug, row.sequence_number);
        let edges = db.list_edges_for_task(&task_id)?;
        for end in edges
            .iter()
            .flat_map(|e| [e.source_task_id, e.target_task_id])
        {
            if !human.contains_key(&end)
                && let Some(t) = db.get_task(&end.to_string())?
            {
                human.insert(end, format!("{}/{}", t.project_slug, t.sequence_number));
            }
        }

        let mut task = TaskOutput::from(row);
        task.messages = db.get_conversation_messages(&task_id)?;
        let env = record::RecordEnvelope {
            task,
            edges: record::record_edges(&edges, &human),
        };
        out.push(RecordFile {
            filename,
            bytes: record::serialize_record(&env),
        });
    }
    Ok(out)
}

/// Build the query filter export uses: the resolved subtree, every status
/// (PRD I3), and an unbounded page. `list_tasks` otherwise caps at
/// `DEFAULT_PAGE_LIMIT` (100), which would silently truncate the manifest.
fn export_filter(project_ids: Vec<Uuid>) -> TaskQueryFilter {
    TaskQueryFilter {
        project_ids: Some(project_ids),
        limit: Some(i64::MAX),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{CreateTaskParams, TaskUpdates};
    use crate::export::test_support::unique_dir;
    use crate::state::TaskStatus;

    fn make_task(db: &Db, project_id: Uuid, project_slug: &str, title: &str) -> TaskRow {
        db.create_task(
            CreateTaskParams {
                project_id,
                project_slug: project_slug.to_string(),
                title: title.to_string(),
                description: String::new(),
                category: None,
                status: None,
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
        .unwrap()
    }

    /// Jump a task straight to `status`, bypassing transition validation — the
    /// records layer only cares about terminal membership, not how it got there.
    fn set_status(db: &Db, id: &Uuid, status: TaskStatus) {
        db.update_task(
            &id.to_string(),
            TaskUpdates {
                status: Some(status),
                force_status: true,
                ..Default::default()
            },
            "test",
        )
        .unwrap();
    }

    fn record_names(db: &Db) -> Vec<String> {
        let mut names: Vec<String> = collect_record_files(db, subtree(db))
            .unwrap()
            .into_iter()
            .map(|r| r.filename)
            .collect();
        names.sort();
        names
    }

    /// End-to-end over a real DB subtree: descendant expansion -> export_filter
    /// -> list_tasks -> serialize_manifest. Covers the seam that serialize_lines
    /// unit tests bypass (line_of on real TaskRows, the i64::MAX limit override,
    /// numeric ordering across the 9/10 boundary, cross-project determinism).
    #[test]
    fn serializes_full_subtree_deterministically() {
        let db = Db::open_in_memory().unwrap();
        let root = db.create_project("root", "Root", "", None, "test").unwrap();
        let child = db
            .create_project("child", "Child", "", Some(root.id), "test")
            .unwrap();

        // Span the 9/10 lexical trap within one project.
        for n in 1..=11 {
            make_task(&db, root.id, &root.slug, &format!("root task {n}"));
        }
        make_task(&db, child.id, &child.slug, "child task");

        let ids = binding::resolve_project_ids(&db, "root").unwrap();
        let tasks = db.list_tasks(&export_filter(ids)).unwrap();
        let bytes = manifest::serialize_manifest(&tasks);
        let text = String::from_utf8(bytes.clone()).unwrap();

        // Descendant workstream is included; nothing truncated.
        assert!(
            text.contains("\"id\":\"child/1\""),
            "child task missing:\n{text}"
        );
        assert_eq!(
            text.lines().count(),
            12,
            "expected 11 root + 1 child:\n{text}"
        );

        // Numeric order, not lexical: root/9 precedes root/10.
        let nine = text.find("\"id\":\"root/9\"").unwrap();
        let ten = text.find("\"id\":\"root/10\"").unwrap();
        assert!(nine < ten, "root/9 must precede root/10:\n{text}");

        // Determinism (I2): an independent re-query serializes byte-identically.
        let reids = binding::resolve_project_ids(&db, "root").unwrap();
        let retasks = db.list_tasks(&export_filter(reids)).unwrap();
        assert_eq!(bytes, manifest::serialize_manifest(&retasks));
    }

    fn export_db() -> (Db, Uuid, String) {
        let db = Db::open_in_memory().unwrap();
        let root = db.create_project("root", "Root", "", None, "test").unwrap();
        (db, root.id, root.slug)
    }

    fn subtree(db: &Db) -> Vec<TaskRow> {
        let ids = binding::resolve_project_ids(db, "root").unwrap();
        db.list_tasks(&export_filter(ids)).unwrap()
    }

    /// AC1 (I4/I5): records only for terminal tasks. A done and a wontfix task
    /// each get a file; open tasks (in-progress, ready-for-agent) do not.
    #[test]
    fn records_only_for_terminal_tasks() {
        let (db, pid, slug) = export_db();
        let done = make_task(&db, pid, &slug, "done one");
        let wontfix = make_task(&db, pid, &slug, "wontfix one");
        let wip = make_task(&db, pid, &slug, "in progress");
        make_task(&db, pid, &slug, "still queued"); // ready-for-agent/needs-triage
        set_status(&db, &done.id, TaskStatus::Done);
        set_status(&db, &wontfix.id, TaskStatus::WontFix);
        set_status(&db, &wip.id, TaskStatus::InProgress);

        assert_eq!(
            record_names(&db),
            vec!["root-1.json".to_string(), "root-2.json".to_string()],
        );
    }

    /// AC3 (story 8): a comment appended after a task is done shows up in its
    /// record on the next collect — the operator's done-then-review workflow.
    #[test]
    fn record_updates_after_post_close_comment() {
        let (db, pid, slug) = export_db();
        let t = make_task(&db, pid, &slug, "review me");
        set_status(&db, &t.id, TaskStatus::Done);

        let before = collect_record_files(&db, subtree(&db)).unwrap();
        db.append_conversation_message(&t.id, "LGTM, shipped", Some("josh"))
            .unwrap();
        let after = collect_record_files(&db, subtree(&db)).unwrap();

        assert_ne!(
            before[0].bytes, after[0].bytes,
            "record did not pick up the note"
        );
        let text = String::from_utf8(after[0].bytes.clone()).unwrap();
        assert!(
            text.contains("LGTM, shipped"),
            "note missing from record:\n{text}"
        );
    }

    /// AC4 (I11): a record written on run 1 is dropped on run 2 once its task
    /// leaves terminal (a reopen), and the still-terminal record survives.
    #[test]
    fn reconcile_removes_record_when_task_leaves_terminal() {
        let (db, pid, slug) = export_db();
        let reopened = make_task(&db, pid, &slug, "will reopen");
        let stays = make_task(&db, pid, &slug, "stays done");
        set_status(&db, &reopened.id, TaskStatus::Done);
        set_status(&db, &stays.id, TaskStatus::Done);
        let root = unique_dir();

        // Run 1: both terminal -> both records on disk.
        let run1 = collect_record_files(&db, subtree(&db)).unwrap();
        let expected1: HashSet<String> = run1.iter().map(|r| r.filename.clone()).collect();
        writer::write_records(&root, &run1).unwrap();
        writer::reconcile_records(&root, &expected1).unwrap();
        let records_dir = root.join(".yojana").join("records");
        assert!(records_dir.join("root-1.json").exists());
        assert!(records_dir.join("root-2.json").exists());

        // Run 2: reopen root-1 -> only root-2 remains terminal.
        set_status(&db, &reopened.id, TaskStatus::NeedsTriage);
        let run2 = collect_record_files(&db, subtree(&db)).unwrap();
        let expected2: HashSet<String> = run2.iter().map(|r| r.filename.clone()).collect();
        writer::write_records(&root, &run2).unwrap();
        writer::reconcile_records(&root, &expected2).unwrap();
        assert!(
            !records_dir.join("root-1.json").exists(),
            "reopened record kept"
        );
        assert!(
            records_dir.join("root-2.json").exists(),
            "live record dropped"
        );
    }

    /// AC6 (I2): re-collecting against an unchanged DB is byte-identical,
    /// including a task carrying an incident edge and a conversation message.
    #[test]
    fn records_layer_deterministic() {
        let (db, pid, slug) = export_db();
        let a = make_task(&db, pid, &slug, "task a");
        let b = make_task(&db, pid, &slug, "task b");
        set_status(&db, &a.id, TaskStatus::Done);
        set_status(&db, &b.id, TaskStatus::Done);
        db.create_edge(a.id, b.id, "depends_on", None).unwrap();
        db.append_conversation_message(&a.id, "note", None).unwrap();

        let first = collect_record_files(&db, subtree(&db)).unwrap();
        let second = collect_record_files(&db, subtree(&db)).unwrap();
        let bytes = |v: &[RecordFile]| v.iter().map(|r| r.bytes.clone()).collect::<Vec<_>>();
        assert_eq!(bytes(&first), bytes(&second));

        // The edge endpoint resolved to a human id, not a raw UUID.
        let text = String::from_utf8(first[0].bytes.clone()).unwrap();
        assert!(
            text.contains("\"target\": \"root/2\""),
            "edge not resolved:\n{text}"
        );
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A fresh, unique temp directory for a filesystem test.
    ///
    /// Tests run as threads in one process, so the clock alone does not separate
    /// them — two threads can read the same nanosecond. The atomic counter
    /// guarantees intra-run uniqueness; the process-lifetime run id avoids
    /// colliding with stale directories from an earlier run under a recycled pid.
    pub fn unique_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        static RUN_ID: std::sync::LazyLock<u128> = std::sync::LazyLock::new(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("invariant: system clock is after the unix epoch")
                .as_nanos()
        });
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "yojana-export-{}-{}-{}",
            std::process::id(),
            *RUN_ID,
            n
        ));
        std::fs::create_dir_all(&dir).expect("invariant: temp dir is creatable");
        dir
    }
}
