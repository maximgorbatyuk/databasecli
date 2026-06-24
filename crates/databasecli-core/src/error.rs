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

    #[error(
        "multi-statement queries (containing ';') are not allowed — submit one statement at a time"
    )]
    MultiStatement,

    #[error("empty query")]
    EmptyQuery,

    #[error("invalid output format '{0}': expected table, csv, tsv, json, or ndjson")]
    InvalidOutputFormat(String),

    #[error(
        "invalid statement_timeout '{0}': expected 0/disable or a positive duration like 30s, 500ms, 5min, 1h"
    )]
    InvalidStatementTimeout(String),

    #[error(
        "invalid schema '{0}': expected one or more comma-separated identifiers like 'analytics' or 'analytics, public'"
    )]
    InvalidSearchPath(String),

    #[error("invalid identifier '{0}': must match [a-zA-Z_][a-zA-Z0-9_]*")]
    InvalidIdentifier(String),

    #[error("table not found: {schema}.{table}")]
    TableNotFound { schema: String, table: String },

    #[error("invalid interval '{0}': expected day, week, month, or year")]
    InvalidInterval(String),

    #[error(
        "statement not supported by `exec` (v1): {0}. Multi-statement input, WITH, and procedural bodies (DO $$ ... $$) are not allowed."
    )]
    UnsupportedExecStatement(String),

    #[error("destructive statement requires confirmation; pass --yes or run interactively")]
    ExecConfirmationRequired,

    #[error("execution cancelled")]
    ExecCancelled,

    #[error("database error: {0}")]
    Postgres(#[from] postgres::Error),
}
