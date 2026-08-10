use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::YojanaError;

/// Task lifecycle status. Single source of truth for status values;
/// string representations exist only at the SQLite and MCP boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    NeedsTriage,
    NeedsInfo,
    ReadyForAgent,
    ReadyForHuman,
    InProgress,
    NeedsReview,
    Done,
    /// Wire format is "wontfix" (no hyphen), not the kebab-case "wont-fix".
    #[serde(rename = "wontfix")]
    WontFix,
}

impl TaskStatus {
    /// All variants, for exhaustive iteration in tests and schema generation.
    /// Manually maintained — the round-trip test catches a missing entry.
    pub const ALL: &[TaskStatus] = &[
        TaskStatus::NeedsTriage,
        TaskStatus::NeedsInfo,
        TaskStatus::ReadyForAgent,
        TaskStatus::ReadyForHuman,
        TaskStatus::InProgress,
        TaskStatus::NeedsReview,
        TaskStatus::Done,
        TaskStatus::WontFix,
    ];

    /// Canonical string representation. Matches the DB TEXT column and MCP wire format.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeedsTriage => "needs-triage",
            Self::NeedsInfo => "needs-info",
            Self::ReadyForAgent => "ready-for-agent",
            Self::ReadyForHuman => "ready-for-human",
            Self::InProgress => "in-progress",
            Self::NeedsReview => "needs-review",
            Self::Done => "done",
            Self::WontFix => "wontfix",
        }
    }

    /// Valid transitions from this status. Exhaustive match, no fallback arm —
    /// adding a variant is a compile error here.
    pub fn transitions(self) -> &'static [TaskStatus] {
        use TaskStatus::*;
        match self {
            NeedsTriage => &[NeedsInfo, ReadyForAgent, ReadyForHuman, InProgress, WontFix],
            NeedsInfo => &[NeedsTriage, ReadyForAgent, ReadyForHuman, WontFix],
            ReadyForAgent => &[NeedsTriage, InProgress, ReadyForHuman],
            ReadyForHuman => &[NeedsTriage, InProgress, ReadyForAgent],
            InProgress => &[NeedsReview, Done, NeedsTriage, WontFix],
            NeedsReview => &[Done, InProgress, NeedsTriage],
            Done => &[NeedsTriage],
            WontFix => &[NeedsTriage],
        }
    }

    /// Whether this status is terminal (done or wontfix).
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::WontFix)
    }
}

// --- String conversions ---

/// The one fallible parse: arbitrary string → TaskStatus.
/// Used at the MCP boundary and by FromSql (in db.rs).
impl FromStr for TaskStatus {
    type Err = YojanaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "needs-triage" => Ok(Self::NeedsTriage),
            "needs-info" => Ok(Self::NeedsInfo),
            "ready-for-agent" => Ok(Self::ReadyForAgent),
            "ready-for-human" => Ok(Self::ReadyForHuman),
            "in-progress" => Ok(Self::InProgress),
            "needs-review" => Ok(Self::NeedsReview),
            "done" => Ok(Self::Done),
            "wontfix" => Ok(Self::WontFix),
            unknown => Err(YojanaError::InvalidInput(format!(
                "unknown status '{unknown}'"
            ))),
        }
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// Note: ToSql/FromSql impls live in db.rs (rusqlite confinement boundary).

/// Validate a status transition. Returns Ok for no-op (from == to).
/// Exhaustive match over `TaskStatus`, no wildcard fallback.
pub fn validate_transition(from: TaskStatus, to: TaskStatus) -> Result<(), YojanaError> {
    if from == to {
        return Ok(());
    }
    if from.transitions().contains(&to) {
        Ok(())
    } else {
        Err(YojanaError::InvalidInput(format!(
            "invalid transition: '{}' → '{}'",
            from, to
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// serde rename == as_str for every variant (the round-trip invariant).
    /// Also guards ALL: a missing entry means the variant isn't tested.
    #[test]
    fn serde_rename_matches_as_str() {
        for &status in TaskStatus::ALL {
            let json = serde_json::to_value(status).expect("invariant: TaskStatus serializes");
            assert_eq!(
                json.as_str().expect("invariant: serializes to string"),
                status.as_str(),
                "serde rename mismatch for {status:?}"
            );
        }
    }

    /// FromStr round-trips with as_str for every variant.
    #[test]
    fn from_str_round_trip() {
        for &status in TaskStatus::ALL {
            let parsed: TaskStatus = status
                .as_str()
                .parse()
                .expect("invariant: as_str output parses back");
            assert_eq!(parsed, status);
        }
    }

    /// Unknown strings are rejected by FromStr.
    #[test]
    fn from_str_rejects_unknown() {
        assert!("bogus".parse::<TaskStatus>().is_err());
        assert!("wont-fix".parse::<TaskStatus>().is_err()); // kebab != wire format
    }

    #[test]
    fn noop_transition_allowed() {
        for &status in TaskStatus::ALL {
            assert!(
                validate_transition(status, status).is_ok(),
                "noop transition should be allowed for {status:?}"
            );
        }
    }

    #[test]
    fn invalid_transitions_rejected() {
        use TaskStatus::*;
        let cases = [
            (Done, InProgress),
            (Done, ReadyForAgent),
            (WontFix, Done),
            (InProgress, ReadyForAgent),
            (ReadyForAgent, Done),
            (NeedsTriage, Done),
        ];
        for (from, to) in cases {
            assert!(
                validate_transition(from, to).is_err(),
                "expected {} → {} to be rejected",
                from,
                to
            );
        }
    }
}
