use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::YojanaError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum RefType {
    #[serde(rename = "smriti:hash")]
    SmritiHash,
    #[serde(rename = "smriti:path")]
    SmritiPath,
    #[serde(rename = "sutra:symbol")]
    SutraSymbol,
    #[serde(rename = "kosha:citation")]
    KoshaCitation,
    #[serde(rename = "yojana:task")]
    YojanaTask,
    #[serde(rename = "chitta:memory")]
    ChittaMemory,
    #[serde(rename = "doc:path")]
    DocPath,
    #[serde(rename = "git:commit")]
    GitCommit,
    #[serde(rename = "git:range")]
    GitRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContextRef {
    #[serde(rename = "type")]
    pub ref_type: RefType,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl ContextRef {
    /// Lenient parse for read-only display paths: malformed JSON reads as
    /// empty. Never use this on a read-modify-write path — see
    /// [`ContextRef::parse_array_strict`].
    pub fn parse_array(json: &str) -> Vec<Self> {
        match serde_json::from_str(json) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("invalid context_refs JSON: {e}");
                Vec::new()
            }
        }
    }

    /// Strict parse for read-modify-write paths: a stored value that fails
    /// to parse is an error. Reading it as empty and writing back would
    /// replace every existing ref (yojana/59).
    pub fn parse_array_strict(stored: &str) -> Result<Vec<Self>, YojanaError> {
        serde_json::from_str(stored).map_err(|e| {
            YojanaError::InvalidInput(format!(
                "stored context_refs is unreadable ({e}); refusing to overwrite it"
            ))
        })
    }

    pub fn git_commit(sha: &str) -> Self {
        Self {
            ref_type: RefType::GitCommit,
            value: sha.to_string(),
            label: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_parse_reads_well_formed_column() {
        let refs = ContextRef::parse_array_strict(r#"[{"type":"doc:path","value":"a.md"}]"#)
            .expect("well-formed column parses");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_type, RefType::DocPath);
    }

    #[test]
    fn strict_parse_rejects_unreadable_column() {
        for bad in [
            r#"{"type":"doc:path","value":"x"}"#,
            "not json",
            r#"[{"type":"nope","value":"x"}]"#,
        ] {
            assert!(
                matches!(
                    ContextRef::parse_array_strict(bad),
                    Err(YojanaError::InvalidInput(_))
                ),
                "{bad} must be rejected"
            );
        }
    }
}
