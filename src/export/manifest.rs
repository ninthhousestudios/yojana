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
            status: task.status.to_string(),
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
    use crate::state::TaskStatus;
    use uuid::Uuid;

    /// A minimal `TaskRow` for exercising `line_of` directly. Only the four
    /// fields the mapping reads (`project_slug`, `sequence_number`, `title`,
    /// `status`, `completed_at`) carry meaning; the rest are inert filler.
    fn task_row(
        slug: &str,
        seq: i64,
        title: &str,
        status: TaskStatus,
        completed_at: Option<i64>,
    ) -> TaskRow {
        TaskRow {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            project_slug: slug.to_string(),
            sequence_number: seq,
            title: title.to_string(),
            description: String::new(),
            category: None,
            status,
            slice_type: None,
            acceptance_criteria: "[]".to_string(),
            decisions: "[]".to_string(),
            implementation_plan: None,
            execution_record: None,
            reproduction: None,
            root_cause: None,
            context_refs: "[]".to_string(),
            files: "[]".to_string(),
            tags: "[]".to_string(),
            history: "[]".to_string(),
            created_at: 0,
            updated_at: 0,
            completed_at,
            arc_id: None,
            arc_phase: None,
        }
    }

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

    #[test]
    fn line_of_maps_taskrow_fields() {
        // The production path (serialize_manifest -> line_of) is the only place
        // real TaskRow fields are read: the {slug}/{seq} id and the
        // completed_at -> closed_at remap. The other tests build ManifestLines
        // by hand and never exercise it.
        let (key, mline) = line_of(&task_row(
            "yojana",
            42,
            "Ship it",
            TaskStatus::Done,
            Some(999),
        ));
        assert_eq!(key, ("yojana".to_string(), 42));
        let bytes = serialize_lines(vec![(key, mline)]);
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "{\"id\":\"yojana/42\",\"title\":\"Ship it\",\"status\":\"done\",\"closed_at\":999}\n"
        );
    }

    #[test]
    fn line_of_open_task_maps_null_closed_at() {
        let (key, mline) = line_of(&task_row("beads", 3, "WIP", TaskStatus::InProgress, None));
        assert_eq!(key, ("beads".to_string(), 3));
        let bytes = serialize_lines(vec![(key, mline)]);
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "{\"id\":\"beads/3\",\"title\":\"WIP\",\"status\":\"in-progress\",\"closed_at\":null}\n"
        );
    }
}
