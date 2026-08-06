//! Pure record serialization: a terminal task's full envelope -> pretty JSON.
//!
//! The record body IS the existing `TaskOutput` envelope (src/tools/task.rs),
//! flattened, so no column can silently drift out of the record (PRD I13 —
//! faithful whole-envelope dump, not a curated projection). The one thing
//! `get` does not carry is incident edges; those are appended here, each
//! resolved to the *other end's* human id so a record resolves offline.
//!
//! No I/O and no DB: the endpoint->human_id map and the messages are supplied
//! by the orchestrator, keeping this module pure and its output deterministic
//! (PRD I2). serde_json has no `preserve_order` feature in this crate, so
//! struct fields emit in declaration order and nested `Value` keys sort
//! stably — both deterministic across runs.

use std::collections::HashMap;

use serde::Serialize;
use uuid::Uuid;

use crate::db::EdgeRow;
use crate::tools::task::TaskOutput;

/// One incident edge as it appears in a record. Mirrors the edge tool's
/// `source`/`target` vocabulary, but both endpoints are human ids (`slug/N`)
/// rather than UUIDs, so the record is self-describing without the DB. The
/// "other end's id" (I13) is whichever of `source`/`target` is not this task.
#[derive(Debug, Serialize)]
pub struct RecordEdge {
    pub id: String,
    pub edge_type: String,
    pub source: String,
    pub target: String,
    pub note: Option<String>,
    pub created_at: i64,
}

/// A full task record: the task envelope flattened, plus its incident edges.
#[derive(Debug, Serialize)]
pub struct RecordEnvelope {
    #[serde(flatten)]
    pub task: TaskOutput,
    pub edges: Vec<RecordEdge>,
}

/// `.yojana/records/<slug>-<seq>.json`. The slug qualifier keeps descendant
/// workstreams distinct (`child/1` vs `yojana/1` never collide on disk); PRD
/// I8's `yojana-<n>.json` is the slug=`yojana` case. This is the single source
/// of truth for the name, shared by the writer and by reconcile's expected set.
pub fn record_filename(project_slug: &str, sequence_number: i64) -> String {
    format!("{project_slug}-{sequence_number}.json")
}

/// Map incident `EdgeRow`s (ordered by `created_at` upstream) to `RecordEdge`s,
/// resolving both endpoints through `human`. A UUID missing from the map falls
/// back to its string form — the orchestrator builds the map to cover every
/// endpoint, so that is a belt-and-braces default, not an expected path.
pub fn record_edges(rows: &[EdgeRow], human: &HashMap<Uuid, String>) -> Vec<RecordEdge> {
    let resolve = |u: &Uuid| human.get(u).cloned().unwrap_or_else(|| u.to_string());
    rows.iter()
        .map(|r| RecordEdge {
            id: r.id.to_string(),
            edge_type: r.edge_type.clone(),
            source: resolve(&r.source_task_id),
            target: resolve(&r.target_task_id),
            note: r.note.clone(),
            created_at: r.created_at,
        })
        .collect()
}

/// Serialize a record: pretty-printed for readable PR diffs (PRD I13),
/// newline-terminated, deterministic (PRD I2).
pub fn serialize_record(env: &RecordEnvelope) -> Vec<u8> {
    let mut s = serde_json::to_string_pretty(env)
        .expect("invariant: RecordEnvelope holds only serializable data");
    s.push('\n');
    s.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::TaskRow;

    fn task_row(slug: &str, seq: i64, status: &str) -> TaskRow {
        TaskRow {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            project_slug: slug.to_string(),
            sequence_number: seq,
            title: "Ship it".to_string(),
            description: String::new(),
            category: None,
            status: status.to_string(),
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
            created_at: 10,
            updated_at: 20,
            completed_at: Some(30),
            arc_id: None,
            arc_phase: None,
        }
    }

    fn edge(id: u128, source: Uuid, target: Uuid, kind: &str) -> EdgeRow {
        EdgeRow {
            id: Uuid::from_u128(id),
            source_task_id: source,
            target_task_id: target,
            edge_type: kind.to_string(),
            note: None,
            created_at: 5,
        }
    }

    fn envelope(status: &str, edges: Vec<RecordEdge>) -> RecordEnvelope {
        RecordEnvelope {
            task: TaskOutput::from(task_row("yojana", 42, status)),
            edges,
        }
    }

    #[test]
    fn record_edges_resolves_both_ends() {
        let me = Uuid::from_u128(1);
        let up = Uuid::from_u128(2);
        let down = Uuid::from_u128(3);
        let human: HashMap<Uuid, String> = [
            (me, "yojana/42".to_string()),
            (up, "yojana/40".to_string()),
            (down, "yojana/50".to_string()),
        ]
        .into_iter()
        .collect();

        // One outgoing (me -> up) and one incoming (down -> me).
        let rows = vec![
            edge(10, me, up, "depends_on"),
            edge(11, down, me, "depends_on"),
        ];
        let edges = record_edges(&rows, &human);

        assert_eq!(edges[0].source, "yojana/42");
        assert_eq!(edges[0].target, "yojana/40");
        assert_eq!(edges[1].source, "yojana/50");
        assert_eq!(edges[1].target, "yojana/42");
    }

    #[test]
    fn record_edges_falls_back_to_uuid_for_unknown_endpoint() {
        let me = Uuid::from_u128(1);
        let stranger = Uuid::from_u128(9);
        let human: HashMap<Uuid, String> = [(me, "yojana/42".to_string())].into_iter().collect();
        let edges = record_edges(&[edge(10, me, stranger, "refines")], &human);
        assert_eq!(edges[0].source, "yojana/42");
        assert_eq!(edges[0].target, stranger.to_string());
    }

    #[test]
    fn filename_is_slug_qualified() {
        assert_eq!(record_filename("yojana", 9), "yojana-9.json");
        assert_eq!(record_filename("child", 1), "child-1.json");
    }

    #[test]
    fn record_is_newline_terminated() {
        let bytes = serialize_record(&envelope("done", vec![]));
        assert_eq!(bytes.last(), Some(&b'\n'));
    }

    /// Golden faithfulness: the parsed record must equal the whole envelope with
    /// nothing dropped (I13). Compared as a `Value` so a new `TaskOutput` column
    /// forces this expectation to be updated rather than silently omitted, while
    /// key *ordering* is left to the determinism test.
    #[test]
    fn golden_record_envelope_is_faithful() {
        let me = Uuid::from_u128(1);
        let up = Uuid::from_u128(2);
        let human: HashMap<Uuid, String> =
            [(me, "yojana/42".to_string()), (up, "yojana/40".to_string())]
                .into_iter()
                .collect();
        let edges = record_edges(&[edge(10, me, up, "depends_on")], &human);
        let bytes = serialize_record(&envelope("done", edges));
        let got: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        let expected = serde_json::json!({
            "id": Uuid::nil().to_string(),
            "project_id": Uuid::nil().to_string(),
            "project_slug": "yojana",
            "human_id": "yojana/42",
            "sequence_number": 42,
            "title": "Ship it",
            "description": "",
            "category": null,
            "status": "done",
            "slice_type": null,
            "acceptance_criteria": [],
            "decisions": [],
            "implementation_plan": null,
            "execution_record": null,
            "reproduction": null,
            "root_cause": null,
            "context_refs": [],
            "files": [],
            "tags": [],
            "history": [],
            "messages": [],
            "created_at": 10,
            "updated_at": 20,
            "completed_at": 30,
            "edges": [{
                "id": Uuid::from_u128(10).to_string(),
                "edge_type": "depends_on",
                "source": "yojana/42",
                "target": "yojana/40",
                "note": null,
                "created_at": 5,
            }],
        });
        assert_eq!(got, expected, "record envelope drifted:\n{got:#}");
    }

    /// Field order is TaskOutput declaration order, with `edges` last. Guards the
    /// `#[serde(flatten)]` + `to_string_pretty` path against a reordering that
    /// the Value-based golden cannot see.
    #[test]
    fn field_order_is_stable_with_edges_last() {
        let bytes = serialize_record(&envelope("done", vec![]));
        let text = String::from_utf8(bytes).unwrap();
        let pos = |k: &str| text.find(&format!("\"{k}\"")).unwrap();
        assert!(pos("id") < pos("status"));
        assert!(pos("status") < pos("completed_at"));
        assert!(pos("completed_at") < pos("edges"));
    }

    #[test]
    fn deterministic_across_runs() {
        let build = || serialize_record(&envelope("wontfix", vec![]));
        assert_eq!(build(), build());
    }
}
