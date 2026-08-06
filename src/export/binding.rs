//! Binding resolver: `CWD -> (repo_root, ExportConfig, project ids)`.
//!
//! Walks up from a starting directory to find `.yojana/export.toml`, parses it,
//! and expands the declared root project into its descendant subtree. Owns
//! config-schema validation and the walk-up; the DB stays repo-agnostic
//! (PRD I6 — no `repo_path` column).

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::Db;

/// Repo-side export config (`.yojana/export.toml`), PRD I15.
///
/// The full schema is `{ project, records }` (I15), but this slice ships the
/// manifest layer only. serde ignores unknown fields by default, so a config
/// that already carries `records` parses cleanly here; the field is added to
/// this struct when the records slice starts consuming it.
#[derive(Debug, Deserialize)]
pub struct ExportConfig {
    /// Root project slug; its descendant workstreams are auto-included.
    pub project: String,
}

/// A resolved binding: the repo root (the directory containing `.yojana/`) and
/// its parsed config.
pub struct Binding {
    pub repo_root: PathBuf,
    pub config: ExportConfig,
}

/// Walk up from `start` toward the filesystem root looking for
/// `.yojana/export.toml`. The directory containing `.yojana` is the repo root.
pub fn find_config(start: &Path) -> anyhow::Result<Binding> {
    let mut dir = Some(start);
    while let Some(current) = dir {
        let cfg_path = current.join(".yojana").join("export.toml");
        if cfg_path.is_file() {
            let text = std::fs::read_to_string(&cfg_path)
                .with_context(|| format!("reading {}", cfg_path.display()))?;
            let config: ExportConfig =
                toml::from_str(&text).with_context(|| format!("parsing {}", cfg_path.display()))?;
            return Ok(Binding {
                repo_root: current.to_path_buf(),
                config,
            });
        }
        dir = current.parent();
    }
    anyhow::bail!(
        "no .yojana/export.toml found in {} or any ancestor directory",
        start.display()
    )
}

/// Expand the root project slug into the full set of project ids to export: the
/// root plus every descendant workstream (PRD I7).
pub fn resolve_project_ids(db: &Db, root_slug: &str) -> anyhow::Result<Vec<Uuid>> {
    let project = db
        .get_project(None, Some(root_slug))?
        .ok_or_else(|| anyhow::anyhow!("root project '{root_slug}' not found"))?;
    Ok(db.project_ids_with_descendants(&project.id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::test_support::unique_dir;

    fn write_config(root: &Path, body: &str) {
        let yj = root.join(".yojana");
        std::fs::create_dir_all(&yj).unwrap();
        std::fs::write(yj.join("export.toml"), body).unwrap();
    }

    #[test]
    fn walk_up_finds_config_from_nested_cwd() {
        let root = unique_dir();
        write_config(&root, "project = \"yojana\"\n");
        let nested = root.join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();

        let binding = find_config(&nested).unwrap();
        assert_eq!(binding.repo_root, root);
        assert_eq!(binding.config.project, "yojana");
    }

    #[test]
    fn missing_config_errors() {
        let root = unique_dir(); // no .yojana written
        assert!(find_config(&root).is_err());
    }

    #[test]
    fn malformed_config_errors() {
        let root = unique_dir();
        write_config(&root, "this is not = valid toml [[[");
        assert!(find_config(&root).is_err());
    }

    #[test]
    fn unknown_records_key_still_parses() {
        // Forward-compat: a config already carrying the records field (I15)
        // parses cleanly even though this slice does not consume it.
        let root = unique_dir();
        write_config(&root, "project = \"yojana\"\nrecords = false\n");
        let binding = find_config(&root).unwrap();
        assert_eq!(binding.config.project, "yojana");
    }

    #[test]
    fn resolve_expands_descendants() {
        let db = Db::open_in_memory().unwrap();
        let root = db.create_project("root", "Root", "", None, "test").unwrap();
        let child = db
            .create_project("child", "Child", "", Some(root.id), "test")
            .unwrap();

        let ids = resolve_project_ids(&db, "root").unwrap();
        assert!(ids.contains(&root.id));
        assert!(ids.contains(&child.id));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn resolve_unknown_project_errors() {
        let db = Db::open_in_memory().unwrap();
        assert!(resolve_project_ids(&db, "nope").is_err());
    }
}
