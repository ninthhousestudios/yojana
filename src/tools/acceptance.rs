use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Doc comments on these two types would land in the MCP tool schema, which is
// on a token diet (see schema_stays_under_token_budget). Kept as plain comments.
//
// AcceptanceCriterion is the canonical stored shape of a criterion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AcceptanceCriterion {
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

// AcceptanceCriterionInput is a criterion as it arrives from a caller or as it
// may already sit in storage. The MCP tool originally declared these items as
// any JSON value, so both a bare string and a {text, done} object were written.
// Reads accept either shape; writes normalize to AcceptanceCriterion so nothing
// new is stored in the string form.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AcceptanceCriterionInput {
    // Legacy form: the criterion text with no completion state.
    Text(String),
    Object(AcceptanceCriterion),
}

impl From<AcceptanceCriterionInput> for AcceptanceCriterion {
    fn from(input: AcceptanceCriterionInput) -> Self {
        match input {
            AcceptanceCriterionInput::Text(text) => Self { text, done: false },
            AcceptanceCriterionInput::Object(criterion) => criterion,
        }
    }
}

/// Normalize incoming criteria to the canonical stored shape.
pub fn normalize(input: Vec<AcceptanceCriterionInput>) -> Vec<AcceptanceCriterion> {
    input.into_iter().map(Into::into).collect()
}

/// Parse a stored `acceptance_criteria` column, accepting either shape.
///
/// `Err` carries the parse failure so callers can surface it — a value matching
/// neither shape must not be reported as "no criteria".
pub fn parse_stored(json: &str) -> Result<Vec<AcceptanceCriterion>, serde_json::Error> {
    if json.trim().is_empty() {
        return Ok(Vec::new());
    }
    let items: Vec<AcceptanceCriterionInput> = serde_json::from_str(json)?;
    Ok(items.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_string_form() {
        let parsed = parse_stored(r#"["first", "second"]"#).expect("string form is accepted");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text, "first");
        assert!(!parsed[0].done);
    }

    #[test]
    fn parses_object_form() {
        let parsed =
            parse_stored(r#"[{"text":"first","done":true}]"#).expect("object form is accepted");
        assert_eq!(parsed[0].text, "first");
        assert!(parsed[0].done);
    }

    #[test]
    fn object_form_defaults_done_to_false() {
        let parsed = parse_stored(r#"[{"text":"first"}]"#).expect("done is optional");
        assert!(!parsed[0].done);
    }

    #[test]
    fn parses_mixed_array() {
        let parsed =
            parse_stored(r#"["first", {"text":"second","done":true}]"#).expect("mixed is accepted");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1].text, "second");
    }

    #[test]
    fn empty_column_is_no_criteria_not_an_error() {
        assert!(parse_stored("").expect("empty column parses").is_empty());
        assert!(parse_stored("[]").expect("empty array parses").is_empty());
    }

    #[test]
    fn malformed_value_is_an_error() {
        parse_stored(r#"[{"note":"no text field"}]"#).expect_err("unknown object shape rejected");
        parse_stored("[42]").expect_err("bare number rejected");
        parse_stored("not json").expect_err("non-JSON rejected");
    }
}
