pub mod arc;
pub mod context;
pub mod edge;
pub mod project;
pub mod query;
pub mod ready;
pub mod task;

use crate::error::YojanaError;

/// Names of the listed `Option` fields the caller supplied. For double-option
/// fields an explicit null (a clear request) is `Some(None)`, so it counts.
macro_rules! supplied_fields {
    ($args:expr; $($field:ident),+ $(,)?) => {
        [$((stringify!($field), $args.$field.is_some())),+]
            .into_iter()
            .filter_map(|(name, set)| set.then_some(name))
            .collect::<Vec<&'static str>>()
    };
}
pub(crate) use supplied_fields;

/// Reject supplied fields outside `action`'s contract (yojana/61, yojana/62).
/// Multi-action tools share one flat args struct, so without this a field the
/// action doesn't read is dropped and the success ack reads as "applied".
/// `hints` appends guidance when a specific field is rejected.
pub(crate) fn reject_inapplicable_fields(
    action: &str,
    supplied: &[&'static str],
    accepts: impl Fn(&str) -> bool,
    hints: &[(&str, &str)],
) -> Result<(), YojanaError> {
    let rejected: Vec<&str> = supplied.iter().copied().filter(|f| !accepts(f)).collect();
    if rejected.is_empty() {
        return Ok(());
    }
    let mut msg = format!(
        "field(s) not applicable to action={action}: {}",
        rejected.join(", ")
    );
    for (field, hint) in hints {
        if rejected.contains(field) {
            msg.push_str("; ");
            msg.push_str(hint);
        }
    }
    Err(YojanaError::InvalidInput(msg))
}
