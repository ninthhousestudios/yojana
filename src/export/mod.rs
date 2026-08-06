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
mod writer;

use std::path::PathBuf;

use uuid::Uuid;

use crate::config::Config;
use crate::db::{Db, TaskQueryFilter};

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

    println!(
        "wrote {} task(s) to {}",
        tasks.len(),
        binding.repo_root.join(".yojana/manifest.jsonl").display()
    );
    Ok(())
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
    use crate::db::CreateTaskParams;

    fn make_task(db: &Db, project_id: Uuid, project_slug: &str, title: &str) {
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
        .unwrap();
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
