//! Writer: manifest bytes -> filesystem, plus the `.gitattributes` seatbelt.
//!
//! Writes `.yojana/manifest.jsonl` via temp-file + atomic rename so a reader
//! never observes a half-written manifest, and idempotently ensures the
//! `merge=union` attribute (PRD I16) so a false merge conflict on the
//! high-frequency artifact never blocks a merge.

use std::path::Path;

use anyhow::Context;

const GITATTRIBUTES_ENTRY: &str = ".yojana/manifest.jsonl merge=union";

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
}
