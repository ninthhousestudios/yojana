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
pub fn write_records(repo_root: &Path, records: &[RecordFile]) -> anyhow::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let dir = repo_root.join(".yojana").join("records");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    for record in records {
        let tmp = dir.join(format!("{}.tmp", record.filename));
        let final_path = dir.join(&record.filename);
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
/// the directory does not exist (nothing was ever written).
pub fn reconcile_records(repo_root: &Path, expected: &HashSet<String>) -> anyhow::Result<()> {
    let dir = repo_root.join(".yojana").join("records");
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".json") && !expected.contains(&name) {
            std::fs::remove_file(entry.path())
                .with_context(|| format!("removing stale record {name}"))?;
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
