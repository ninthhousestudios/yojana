use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    pub fn parse_array(json: &str) -> Vec<Self> {
        match serde_json::from_str(json) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("invalid context_refs JSON: {e}");
                Vec::new()
            }
        }
    }
}
