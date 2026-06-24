use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "databasecli",
    about = "PostgreSQL database connection manager",
    version,
    after_help = "For a full reference (commands, keys, config, MCP, security), run: databasecli reference"
)]
pub struct Cli {
    /// Working directory to display
    #[arg(short = 'D', long = "directory", global = true)]
    pub directory: Option<String>,

    /// Database names to connect to (from config). Repeatable.
    #[arg(long = "db", global = true)]
    pub databases: Vec<String>,

    /// Connect to all configured databases.
    #[arg(long = "all", global = true)]
    pub all_databases: bool,

    /// Override statement_timeout for this run (e.g. 30s, 500ms, 5min, 1h;
    /// 0/disable turns it off). Defaults to the [settings] value, then 30s.
    #[arg(long = "timeout", global = true)]
    pub timeout: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Launch the interactive TUI
    Tui,
    /// Initialize config and MCP server configuration
    Init,
    /// List all stored database connections (from config)
    List,
    /// Check health of all stored database connections (legacy)
    Health,
    /// List all currently connected databases
    ListDatabases,
    /// Enhanced health check: version, size, uptime
    HealthCheck,
    /// Dump schema: tables, columns, types, PKs, row counts, sizes
    Schema {
        /// Filter by schema name
        #[arg(long, default_value = "public")]
        schema: String,
    },
    /// Run a read-only SQL query
    Query {
        /// The SQL query to execute
        sql: String,
        /// Max rows to return (overrides [settings] query_limit; 0 = unlimited)
        #[arg(long)]
        limit: Option<u32>,
        /// Output format for stdout (summary/timing always go to stderr)
        #[arg(long, default_value = "table", value_parser = ["table", "csv", "tsv", "json", "ndjson"])]
        format: String,
        /// Omit the header row (table/csv/tsv)
        #[arg(long = "no-header")]
        no_header: bool,
    },
    /// Execute write/DDL SQL on one database (local CLI only; not exposed via MCP)
    ///
    /// Inline form runs a single statement. Use `--file` for multi-statement
    /// scripts (BEGIN/COMMIT, WITH ... DML chains, seed files). Procedural
    /// bodies (`DO $$ ... $$`), dollar-quoted strings, and `COPY` are not
    /// supported in either form.
    Exec {
        /// The SQL statement to execute. Single statement only; may be a
        /// `WITH ... <INSERT|UPDATE|DELETE>` chain. Required unless `--file`
        /// is given.
        sql: Option<String>,
        /// Read SQL from a file. Supports multiple `;`-separated statements
        /// including transaction control (BEGIN/COMMIT/ROLLBACK/SAVEPOINT/SET).
        #[arg(short = 'f', long = "file", conflicts_with = "sql")]
        file: Option<String>,
        /// Wrap the file's statements in a single BEGIN/COMMIT transaction.
        /// Use this when your file does not already manage transactions and
        /// you want all-or-nothing rollback on the first failure. Ignored
        /// for inline SQL.
        #[arg(long = "transaction", requires = "file")]
        transaction: bool,
        /// Bypass the destructive-statement confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
    /// Profile a table: nulls, cardinality, distributions, top values
    Analyze {
        /// Table name
        table: String,
        /// Schema name
        #[arg(long, default_value = "public")]
        schema: String,
    },
    /// Database summary: table counts, total rows, largest tables
    Summary,
    /// Entity-relationship diagram: PKs and foreign keys
    Erd {
        /// Schema name
        #[arg(long, default_value = "public")]
        schema: String,
        /// Output format: ascii, mermaid, dot
        #[arg(long, default_value = "ascii")]
        format: String,
        /// Export to file
        #[arg(long)]
        output: Option<String>,
    },
    /// Run same query across all connected databases and compare
    Compare {
        /// The SQL query to execute on all databases
        sql: String,
    },
    /// Time-series trend: counts/averages grouped by interval
    Trend {
        /// Table name
        table: String,
        /// Timestamp column
        #[arg(long)]
        timestamp: String,
        /// Grouping interval: day, week, month, year
        #[arg(long, default_value = "day")]
        interval: String,
        /// Value column for AVG computation
        #[arg(long)]
        value: Option<String>,
        /// Schema name
        #[arg(long, default_value = "public")]
        schema: String,
        /// Limit number of periods
        #[arg(long)]
        limit: Option<i64>,
    },
    /// Show full help reference: commands, keys, config, MCP, security
    #[command(name = "reference")]
    Reference,
    /// Stream a table or read-only query to a file/stdout (csv, jsonl, sql)
    ///
    /// Uses a server-side cursor, so it is not bounded by query_limit and is
    /// safe for very large tables. Read-only; never exposed via MCP.
    Export {
        /// Table to export. Mutually exclusive with --query.
        table: Option<String>,
        /// Export the result of a read-only query instead of a whole table.
        /// `--format sql` is not available in this mode (no single target table).
        #[arg(long = "query", conflicts_with = "table")]
        query: Option<String>,
        /// Output format: csv, jsonl, sql
        #[arg(long, default_value = "csv", value_parser = ["csv", "jsonl", "ndjson", "sql"])]
        format: String,
        /// Write to this file instead of stdout
        #[arg(long)]
        output: Option<String>,
        /// Schema name (table mode)
        #[arg(long, default_value = "public")]
        schema: String,
    },
    /// Preview rows from a table
    Sample {
        /// Table name
        table: String,
        /// Number of rows
        #[arg(long, default_value = "20")]
        limit: i64,
        /// Column to order by (descending)
        #[arg(long)]
        order_by: Option<String>,
        /// Schema name
        #[arg(long, default_value = "public")]
        schema: String,
    },
}
