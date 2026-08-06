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

use crate::config::Config;
use crate::db::{Db, TaskQueryFilter};

/// Run `yojana export`, starting the config walk-up from `cwd`.
pub fn run(cwd: PathBuf) -> anyhow::Result<()> {
    let binding = binding::find_config(&cwd)?;
    let config = Config::from_env();
    let db = Db::open(&config)?;
    let project_ids = binding::resolve_project_ids(&db, &binding.config.project)?;

    let filter = TaskQueryFilter {
        project_ids: Some(project_ids),
        // Export covers ALL tasks regardless of status (PRD I3). list_tasks
        // otherwise caps at DEFAULT_PAGE_LIMIT (100), which would silently
        // truncate the manifest — request an unbounded page.
        limit: Some(i64::MAX),
        ..Default::default()
    };
    let tasks = db.list_tasks(&filter)?;

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
