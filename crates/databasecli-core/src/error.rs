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

    #[error("already connected to '{0}'")]
    AlreadyConnected(String),

    #[error("not connected to '{0}'")]
    NotConnected(String),

    #[error("no active connections")]
    NoConnections,

    #[error("query failed: {0}")]
    QueryFailed(String),

    #[error("read-only violation: statement begins with '{0}' which is not allowed")]
    ReadOnlyViolation(String),

    #[error("empty query")]
    EmptyQuery,

    #[error("invalid identifier '{0}': must match [a-zA-Z_][a-zA-Z0-9_]*")]
    InvalidIdentifier(String),

    #[error("table not found: {schema}.{table}")]
    TableNotFound { schema: String, table: String },

    #[error("invalid interval '{0}': expected day, week, month, or year")]
    InvalidInterval(String),

    #[error("database error: {0}")]
    Postgres(#[from] postgres::Error),
}
