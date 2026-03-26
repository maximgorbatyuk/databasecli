use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum DatabaseCliError {
    #[error("config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("config parse error: {0}")]
    ConfigParse(String),

    #[error("missing field '{field}' in section [{section}]")]
    MissingField { section: String, field: String },

    #[error("invalid port '{value}' in section [{section}]: {reason}")]
    InvalidPort {
        section: String,
        value: String,
        reason: String,
    },

    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("could not determine home directory")]
    NoHomeDirectory,
}
