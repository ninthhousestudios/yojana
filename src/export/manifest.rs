//! Pure, deterministic manifest serialization: `TaskRow -> JSONL bytes`.
//!
//! No I/O. The manifest is the high-frequency reference-resolution artifact
//! (one line per task, every status), so byte-for-byte determinism (PRD I2)
//! and numeric ordering (PRD I9) are the whole job here.

use serde::Serialize;

use crate::db::TaskRow;

/// One manifest line. Field declaration order IS the emitted key order — serde
/// emits struct fields in order — which keeps output stable without pulling in
/// serde_json's `preserve_order` feature. Fields are exactly
/// `{id, title, status, closed_at}` per PRD I14.
#[derive(Debug, Serialize)]
struct ManifestLine {
    id: String,
    title: String,
    status: String,
    /// `completed_at` remapped to `closed_at`; `None` serializes as `null`.
    closed_at: Option<i64>,
}

/// Sort key `(project_slug, sequence_number)`. Numeric on the sequence so
/// `yojana/9` precedes `yojana/10` (PRD I9), rather than sorting the rendered
/// `id` string lexically (where `"yojana/10" < "yojana/9"`).
type SortKey = (String, i64);

fn line_of(task: &TaskRow) -> (SortKey, ManifestLine) {
    (
        (task.project_slug.clone(), task.sequence_number),
        ManifestLine {
            id: format!("{}/{}", task.project_slug, task.sequence_number),
            title: task.title.clone(),
            status: task.status.clone(),
            closed_at: task.completed_at,
        },
    )
}

/// Sort by composite key, then render one compact JSON object per line,
/// newline-terminated. Split out from [`serialize_manifest`] so the ordering
/// and byte-shape guarantees can be tested without constructing `TaskRow`s.
fn serialize_lines(mut lines: Vec<(SortKey, ManifestLine)>) -> Vec<u8> {
    lines.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::new();
    for (_, line) in &lines {
        let json = serde_json::to_string(line)
            .expect("invariant: ManifestLine holds only serializable primitives");
        out.push_str(&json);
        out.push('\n');
    }
    out.into_bytes()
}

/// Serialize every task into the deterministic manifest byte stream.
pub fn serialize_manifest(tasks: &[TaskRow]) -> Vec<u8> {
    serialize_lines(tasks.iter().map(line_of).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(
        slug: &str,
        seq: i64,
        title: &str,
        status: &str,
        closed_at: Option<i64>,
    ) -> (SortKey, ManifestLine) {
        (
            (slug.to_string(), seq),
            ManifestLine {
                id: format!("{slug}/{seq}"),
                title: title.to_string(),
                status: status.to_string(),
                closed_at,
            },
        )
    }

    #[test]
    fn golden_line_shape() {
        let bytes = serialize_lines(vec![line("yojana", 9, "Do a thing", "done", Some(123))]);
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "{\"id\":\"yojana/9\",\"title\":\"Do a thing\",\"status\":\"done\",\"closed_at\":123}\n"
        );
    }

    #[test]
    fn open_task_has_null_closed_at() {
        let bytes = serialize_lines(vec![line("yojana", 1, "Open", "in-progress", None)]);
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "{\"id\":\"yojana/1\",\"title\":\"Open\",\"status\":\"in-progress\",\"closed_at\":null}\n"
        );
    }

    #[test]
    fn sorted_numerically_not_lexically() {
        // Input reversed and lexically misordered: 10 ahead of 9.
        let bytes = serialize_lines(vec![
            line("yojana", 10, "ten", "ready-for-agent", None),
            line("yojana", 9, "nine", "ready-for-agent", None),
        ]);
        let text = String::from_utf8(bytes).unwrap();
        let nine = text.find("yojana/9").unwrap();
        let ten = text.find("yojana/10").unwrap();
        assert!(nine < ten, "yojana/9 must precede yojana/10:\n{text}");
    }

    #[test]
    fn deterministic_across_runs() {
        let build = || {
            serialize_lines(vec![
                line("yojana", 2, "b", "in-progress", None),
                line("beads", 5, "a", "done", Some(1)),
                line("yojana", 1, "c", "wontfix", Some(2)),
            ])
        };
        assert_eq!(build(), build());
    }
}
