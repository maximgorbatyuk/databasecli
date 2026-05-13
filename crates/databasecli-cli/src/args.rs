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
