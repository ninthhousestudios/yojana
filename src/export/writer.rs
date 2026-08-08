//! Writer: manifest bytes -> filesystem, plus the `.gitattributes` seatbelt.
//!
//! Writes `.yojana/manifest.jsonl` via temp-file + atomic rename so a reader
//! never observes a half-written manifest, and idempotently ensures the
//! `merge=union` attribute (PRD I16) so a false merge conflict on the
//! high-frequency artifact never blocks a merge.

use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;

const GITATTRIBUTES_ENTRY: &str = ".yojana/manifest.jsonl merge=union";

/// One record file to write: its name (`record::record_filename`) and bytes.
pub struct RecordFile {
    pub filename: String,
    pub bytes: Vec<u8>,
}

/// Write the manifest atomically: full bytes to a temp file in the same
/// directory, then rename over the destination (atomic on a single filesystem).
pub fn write_manifest(repo_root: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let dir = repo_root.join(".yojana");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let tmp = dir.join("manifest.jsonl.tmp");
    let final_path = dir.join("manifest.jsonl");
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, &final_path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), final_path.display()))?;
    Ok(())
}

/// Ensure the repo's `.gitattributes` carries the `merge=union` entry for the
/// manifest. Idempotent: a no-op when the exact entry is already present, and
/// it preserves any prior content.
pub fn ensure_gitattributes(repo_root: &Path) -> anyhow::Result<()> {
    let path = repo_root.join(".gitattributes");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing
        .lines()
        .any(|line| line.trim() == GITATTRIBUTES_ENTRY)
    {
        return Ok(());
    }
    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(GITATTRIBUTES_ENTRY);
    next.push('\n');
    std::fs::write(&path, &next).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Write each record under `.yojana/records/` via temp-file + atomic rename, so
/// a reader never observes a half-written record. A no-op for an empty batch —
/// the directory is only created when there is at least one record to write.
/// A record's filename embeds its slug (`sutra/needs-designing-15.json`), so a
/// descendant workstream lands in a subdirectory that must be created first.
pub fn write_records(repo_root: &Path, records: &[RecordFile]) -> anyhow::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let dir = repo_root.join(".yojana").join("records");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    for record in records {
        let final_path = dir.join(&record.filename);
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let tmp = final_path.with_extension("json.tmp");
        std::fs::write(&tmp, &record.bytes)
            .with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &final_path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), final_path.display()))?;
    }
    Ok(())
}

/// Drop record files no longer backed by a terminal task (PRD I11 — records
/// reflect *current* terminal membership). Conservative: only `.json` files
/// under `records/` that are absent from `expected` are removed. A no-op when
/// the directory does not exist (nothing was ever written). `expected` keys are
/// slug-qualified filenames (`sutra/needs-designing-15.json`), so the walk must
/// descend into per-slug subdirectories and match on the path relative to
/// `records/`, not the bare file name.
pub fn reconcile_records(repo_root: &Path, expected: &HashSet<String>) -> anyhow::Result<()> {
    let dir = repo_root.join(".yojana").join("records");
    if !dir.is_dir() {
        return Ok(());
    }
    reconcile_dir(&dir, &dir, expected)
}

/// Recurse `current` (under `root`), removing stale `.json` records. The name
/// compared against `expected` is the slash-joined path relative to `root`, to
/// mirror `record_filename`'s slug-qualified form.
fn reconcile_dir(root: &Path, current: &Path, expected: &HashSet<String>) -> anyhow::Result<()> {
    for entry in
        std::fs::read_dir(current).with_context(|| format!("reading {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            reconcile_dir(root, &path, expected)?;
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .expect("invariant: walked path lies under records/ root");
        let name = rel
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        if name.ends_with(".json") && !expected.contains(&name) {
            std::fs::remove_file(&path).with_context(|| format!("removing stale record {name}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::test_support::unique_dir;

    #[test]
    fn write_manifest_is_exact_and_overwrites() {
        let root = unique_dir();
        let manifest = root.join(".yojana").join("manifest.jsonl");

        write_manifest(&root, b"line1\n").unwrap();
        assert_eq!(std::fs::read(&manifest).unwrap(), b"line1\n");

        write_manifest(&root, b"line2\n").unwrap();
        assert_eq!(std::fs::read(&manifest).unwrap(), b"line2\n");
    }

    #[test]
    fn ensure_gitattributes_is_idempotent() {
        let root = unique_dir();
        ensure_gitattributes(&root).unwrap();
        ensure_gitattributes(&root).unwrap();

        let content = std::fs::read_to_string(root.join(".gitattributes")).unwrap();
        let count = content
            .lines()
            .filter(|l| l.trim() == GITATTRIBUTES_ENTRY)
            .count();
        assert_eq!(count, 1, "entry must appear exactly once:\n{content}");
    }

    #[test]
    fn ensure_gitattributes_preserves_existing_content() {
        let root = unique_dir();
        std::fs::write(root.join(".gitattributes"), "*.rs text\n").unwrap();

        ensure_gitattributes(&root).unwrap();

        let content = std::fs::read_to_string(root.join(".gitattributes")).unwrap();
        assert!(
            content.contains("*.rs text"),
            "prior content lost:\n{content}"
        );
        assert!(content.lines().any(|l| l.trim() == GITATTRIBUTES_ENTRY));
    }

    fn record(name: &str, body: &[u8]) -> RecordFile {
        RecordFile {
            filename: name.to_string(),
            bytes: body.to_vec(),
        }
    }

    #[test]
    fn write_records_writes_and_overwrites() {
        let root = unique_dir();
        let path = root.join(".yojana").join("records").join("yojana-1.json");

        write_records(&root, &[record("yojana-1.json", b"v1")]).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"v1");

        write_records(&root, &[record("yojana-1.json", b"v2")]).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"v2");
    }

    #[test]
    fn write_records_creates_subproject_dir() {
        let root = unique_dir();
        let path = root
            .join(".yojana")
            .join("records")
            .join("sutra")
            .join("needs-designing-15.json");

        write_records(&root, &[record("sutra/needs-designing-15.json", b"v1")]).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"v1");
    }

    #[test]
    fn reconcile_handles_subproject_records() {
        let root = unique_dir();
        write_records(
            &root,
            &[
                record("sutra/needs-designing-1.json", b"a"),
                record("sutra/needs-designing-2.json", b"b"),
            ],
        )
        .unwrap();

        let expected: HashSet<String> = ["sutra/needs-designing-2.json".to_string()]
            .into_iter()
            .collect();
        reconcile_records(&root, &expected).unwrap();

        let sub = root.join(".yojana").join("records").join("sutra");
        assert!(
            !sub.join("needs-designing-1.json").exists(),
            "stale subproject record kept"
        );
        assert!(
            sub.join("needs-designing-2.json").exists(),
            "live subproject record dropped"
        );
    }

    #[test]
    fn write_records_empty_batch_creates_nothing() {
        let root = unique_dir();
        write_records(&root, &[]).unwrap();
        assert!(!root.join(".yojana").join("records").exists());
    }

    /// The one required reconcile test (PRD I11): a record present on run 1,
    /// no longer terminal on run 2, is absent afterward — while a still-terminal
    /// record is preserved.
    #[test]
    fn reconcile_drops_stale_keeps_expected() {
        let root = unique_dir();
        write_records(
            &root,
            &[record("yojana-1.json", b"a"), record("yojana-2.json", b"b")],
        )
        .unwrap();

        let expected: HashSet<String> = ["yojana-2.json".to_string()].into_iter().collect();
        reconcile_records(&root, &expected).unwrap();

        let records = root.join(".yojana").join("records");
        assert!(!records.join("yojana-1.json").exists(), "stale record kept");
        assert!(
            records.join("yojana-2.json").exists(),
            "live record dropped"
        );
    }

    #[test]
    fn reconcile_noop_when_dir_absent() {
        let root = unique_dir();
        reconcile_records(&root, &HashSet::new()).unwrap();
    }

    #[test]
    fn reconcile_ignores_non_json() {
        let root = unique_dir();
        let records = root.join(".yojana").join("records");
        std::fs::create_dir_all(&records).unwrap();
        std::fs::write(records.join("README.md"), b"notes").unwrap();

        reconcile_records(&root, &HashSet::new()).unwrap();

        assert!(
            records.join("README.md").exists(),
            "non-record file removed"
        );
    }
}
