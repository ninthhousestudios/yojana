use crate::error::YojanaError;

const VALID_STATUSES: &[&str] = &[
    "needs-triage",
    "needs-info",
    "ready-for-agent",
    "ready-for-human",
    "in-progress",
    "done",
    "wontfix",
];

pub fn valid_status(s: &str) -> bool {
    VALID_STATUSES.contains(&s)
}

pub fn validate_transition(from: &str, to: &str) -> Result<(), YojanaError> {
    if from == to {
        return Ok(());
    }

    if !valid_status(from) {
        return Err(YojanaError::InvalidInput(format!(
            "unknown status '{from}'"
        )));
    }
    if !valid_status(to) {
        return Err(YojanaError::InvalidInput(format!("unknown status '{to}'")));
    }

    let allowed: &[&str] = match from {
        "needs-triage" => &[
            "needs-info",
            "ready-for-agent",
            "ready-for-human",
            "in-progress",
            "wontfix",
        ],
        "needs-info" => &[
            "needs-triage",
            "ready-for-agent",
            "ready-for-human",
            "wontfix",
        ],
        "ready-for-agent" => &["in-progress", "needs-triage", "ready-for-human"],
        "ready-for-human" => &["in-progress", "needs-triage", "ready-for-agent"],
        "in-progress" => &["done", "needs-triage", "wontfix"],
        "done" => &["needs-triage"],
        "wontfix" => &["needs-triage"],
        _ => &[],
    };

    if allowed.contains(&to) {
        Ok(())
    } else {
        Err(YojanaError::InvalidInput(format!(
            "invalid transition: '{from}' → '{to}'"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_transition_allowed() {
        assert!(validate_transition("done", "done").is_ok());
    }

    #[test]
    fn valid_forward_transitions() {
        let cases = [
            ("needs-triage", "needs-info"),
            ("needs-triage", "ready-for-agent"),
            ("needs-triage", "ready-for-human"),
            ("needs-triage", "in-progress"),
            ("needs-triage", "wontfix"),
            ("needs-info", "needs-triage"),
            ("needs-info", "ready-for-agent"),
            ("needs-info", "ready-for-human"),
            ("needs-info", "wontfix"),
            ("ready-for-agent", "in-progress"),
            ("ready-for-agent", "needs-triage"),
            ("ready-for-agent", "ready-for-human"),
            ("ready-for-human", "in-progress"),
            ("ready-for-human", "needs-triage"),
            ("ready-for-human", "ready-for-agent"),
            ("in-progress", "done"),
            ("in-progress", "needs-triage"),
            ("in-progress", "wontfix"),
            ("done", "needs-triage"),
            ("wontfix", "needs-triage"),
        ];
        for (from, to) in cases {
            assert!(
                validate_transition(from, to).is_ok(),
                "expected {from} → {to} to be valid"
            );
        }
    }

    #[test]
    fn invalid_transitions_rejected() {
        let cases = [
            ("done", "in-progress"),
            ("done", "ready-for-agent"),
            ("wontfix", "done"),
            ("in-progress", "ready-for-agent"),
            ("ready-for-agent", "done"),
            ("needs-triage", "done"),
        ];
        for (from, to) in cases {
            assert!(
                validate_transition(from, to).is_err(),
                "expected {from} → {to} to be rejected"
            );
        }
    }

    #[test]
    fn unknown_status_rejected() {
        assert!(validate_transition("needs-triage", "bogus").is_err());
        assert!(validate_transition("bogus", "done").is_err());
    }
}
