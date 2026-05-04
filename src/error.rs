use thiserror::Error;

#[derive(Debug, Error)]
pub enum YojanaError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl YojanaError {
    pub fn code(&self) -> i32 {
        match self {
            Self::NotFound(_) => -32001,
            Self::Conflict(_) => -32002,
            Self::InvalidInput(_) => -32003,
            Self::Db(_) | Self::Json(_) => -32000,
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }
}
