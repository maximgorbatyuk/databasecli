# Design: SQL Commands for databasecli

## 1. Overview

This design adds 10 SQL commands and a persistent connection layer, transforming databasecli from a connection-listing/health-checking tool into a full read-only PostgreSQL exploration utility. Each command is available both as a CLI subcommand and a TUI screen.

The central new concept is a **ConnectionManager** -- a persistent session layer where the user explicitly selects which configured databases to connect to. All commands operate only on connected databases. Read-only access is enforced at two layers: client-side SQL parsing and PostgreSQL session-level `SET default_transaction_read_only = on`.

No async runtime is introduced. Heavy operations use `std::thread` + `mpsc::channel`, following the existing TUI health check pattern.

---

## 2. Connection Manager

### 2.1 Core Struct

New file: `crates/databasecli-core/src/connection.rs`

```rust
pub struct LiveConnection {
    pub config: DatabaseConfig,
    pub client: postgres::Client,
}

pub struct ConnectionManager {
    connections: HashMap<String, LiveConnection>,  // keyed by config name
}
```

**Key methods:**
- `connect(&mut self, config: &DatabaseConfig) -> Result<()>` -- opens connection, runs `SET default_transaction_read_only = on`
- `disconnect(&mut self, name: &str) -> Result<()>`
- `disconnect_all(&mut self)`
- `connected_names(&self) -> Vec<String>`
- `get_mut(&mut self, name: &str) -> Option<&mut LiveConnection>`
- `iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut LiveConnection)>`

### 2.2 Thread Safety

`postgres::Client` is `Send` but not `Sync`. The TUI wraps the manager in `Arc<Mutex<ConnectionManager>>`. Background threads clone the `Arc`, lock the mutex, run queries, and unlock. The main TUI thread never locks during rendering -- it only reads cached results from `AppState`.

For CLI mode: `ConnectionManager` is owned directly on the main thread (no `Arc<Mutex<>>` needed).

### 2.3 CLI vs TUI Usage

**CLI mode:** The `--db <name>` flag (repeatable) or `--all` selects databases. Connection is established, command runs, process exits.

```
databasecli schema --db production
databasecli query --db production --db staging "SELECT count(*) FROM users"
databasecli compare --all "SELECT count(*) FROM users"
```

**TUI mode:** A "Connect" screen shows configured databases as a checklist. User toggles which to connect. Once connected, command screens operate on the connected set.

### 2.4 Connect Implementation

```rust
pub fn connect(&mut self, config: &DatabaseConfig) -> Result<(), DatabaseCliError> {
    // TLS connector (accept self-signed)
    let connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    let connector = postgres_native_tls::MakeTlsConnector::new(connector);

    let mut client = postgres::Client::connect(&config.connection_string(), connector)?;

    // Enforce read-only at session level
    client.batch_execute("SET default_transaction_read_only = on")?;

    self.connections.insert(config.name.clone(), LiveConnection {
        config: config.clone(),
        client,
    });
    Ok(())
}
```

---

## 3. SQL Commands

All command logic lives in `crates/databasecli-core/src/commands/`. Each command gets its own module file.

```
crates/databasecli-core/src/commands/
  mod.rs               -- re-exports, shared utilities (validate_identifier)
  list_databases.rs
  health.rs
  schema.rs
  query.rs
  analyze.rs
  compare.rs
  summary.rs
  trend.rs
  sample.rs
  erd.rs
```

### 3.1 list_databases

Show all currently connected databases (not configured -- only connected).

**SQL:** None. Reads from `ConnectionManager` state.

```rust
pub struct ConnectedDatabase {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub user: String,
}

pub fn list_connected(manager: &ConnectionManager) -> Vec<ConnectedDatabase>;
pub fn format_connected_table(databases: &[ConnectedDatabase]) -> String;
```

### 3.2 health (enhanced)

Check connectivity, PostgreSQL version, database size, uptime.

**SQL:**
```sql
SELECT version();
SELECT pg_size_pretty(pg_database_size(current_database())) AS size;
SELECT now() - pg_postmaster_start_time() AS uptime;
```

```rust
pub struct EnhancedHealthResult {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub status: HealthStatus,
    pub response_time: Option<Duration>,
    pub pg_version: Option<String>,
    pub db_size: Option<String>,
    pub uptime: Option<String>,
    pub error: Option<String>,
}

pub fn check_enhanced_health(conn: &mut LiveConnection) -> EnhancedHealthResult;
pub fn check_all_enhanced_health(manager: &mut ConnectionManager) -> Vec<EnhancedHealthResult>;
```

### 3.3 schema

Full schema dump: tables, columns, types, primary keys, row counts, sizes.

**SQL:**
```sql
-- Tables with row counts and sizes
SELECT schemaname, relname AS table_name, n_live_tup AS row_count,
       pg_size_pretty(pg_total_relation_size(schemaname || '.' || relname)) AS total_size
FROM pg_stat_user_tables ORDER BY schemaname, relname;

-- Columns
SELECT table_schema, table_name, column_name, data_type,
       character_maximum_length, is_nullable, column_default
FROM information_schema.columns
WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
ORDER BY table_schema, table_name, ordinal_position;

-- Primary keys
SELECT tc.table_schema, tc.table_name, kcu.column_name
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu
    ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
WHERE tc.constraint_type = 'PRIMARY KEY'
ORDER BY tc.table_schema, tc.table_name, kcu.ordinal_position;
```

```rust
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub row_count: i64,
    pub total_size: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_key_columns: Vec<String>,
}

pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub max_length: Option<i32>,
    pub is_nullable: bool,
    pub default_value: Option<String>,
}

pub fn dump_schema(conn: &mut LiveConnection, schema_filter: Option<&str>) -> Result<SchemaResult>;
```

### 3.4 query

Run read-only SQL (SELECT, WITH, EXPLAIN, SHOW). Enforced with client-side parsing AND session-level read-only.

```rust
pub struct QueryResultSet {
    pub database_name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,     // all values stringified
    pub row_count: usize,
    pub execution_time: Duration,
}

pub fn validate_readonly(sql: &str) -> Result<()>;
pub fn execute_query(conn: &mut LiveConnection, sql: &str) -> Result<QueryResultSet>;
```

**Client-side validation:** Strip SQL comments, get first keyword. Allow: `SELECT`, `WITH`, `EXPLAIN`, `SHOW`, `TABLE`. Reject everything else.

### 3.5 analyze

Profile a table: nulls, cardinality, distributions, top values.

**SQL (generated per column):**
```sql
SELECT '{col}' AS column_name, COUNT(*) AS total_rows,
       COUNT({col}) AS non_null_count,
       COUNT(*) - COUNT({col}) AS null_count,
       COUNT(DISTINCT {col}) AS distinct_count,
       ROUND(100.0 * (COUNT(*) - COUNT({col})) / NULLIF(COUNT(*), 0), 2) AS null_pct
FROM {schema}.{table};

-- Top N values per column
SELECT {col}::text AS value, COUNT(*) AS freq
FROM {schema}.{table} WHERE {col} IS NOT NULL
GROUP BY {col} ORDER BY freq DESC LIMIT 10;

-- Min/max for numeric/date columns
SELECT MIN({col})::text, MAX({col})::text, AVG({col}::numeric)::text
FROM {schema}.{table};
```

```rust
pub struct ColumnProfile {
    pub name: String,
    pub data_type: String,
    pub total_rows: i64,
    pub non_null_count: i64,
    pub null_count: i64,
    pub null_pct: f64,
    pub distinct_count: i64,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub avg_value: Option<String>,
    pub top_values: Vec<(String, i64)>,
}

pub struct TableProfile {
    pub database_name: String,
    pub schema: String,
    pub table: String,
    pub total_rows: i64,
    pub columns: Vec<ColumnProfile>,
}

pub fn analyze_table(conn: &mut LiveConnection, table: &str, schema: Option<&str>) -> Result<TableProfile>;
```

### 3.6 compare

Same query across ALL connected databases, side by side.

```rust
pub struct CompareResult {
    pub query: String,
    pub results: Vec<QueryResultSet>,
    pub errors: Vec<(String, String)>,  // (db_name, error)
}

pub fn compare_query(manager: &mut ConnectionManager, sql: &str) -> Result<CompareResult>;
```

Iterates all connections sequentially, calls `execute_query` on each.

### 3.7 summary

Overview: table count, total rows, largest tables, database size.

**SQL:**
```sql
SELECT COUNT(*) FROM information_schema.tables
WHERE table_schema NOT IN ('pg_catalog', 'information_schema');

SELECT schemaname || '.' || relname AS table_name, n_live_tup AS row_count,
       pg_total_relation_size(schemaname || '.' || relname) AS total_bytes
FROM pg_stat_user_tables
ORDER BY pg_total_relation_size(schemaname || '.' || relname) DESC;

SELECT pg_size_pretty(pg_database_size(current_database())) AS db_size;

SELECT COUNT(*) FROM pg_indexes
WHERE schemaname NOT IN ('pg_catalog', 'information_schema');
```

```rust
pub struct DatabaseSummary {
    pub database_name: String,
    pub table_count: i64,
    pub total_rows: i64,
    pub total_size: String,
    pub index_count: i64,
    pub largest_tables: Vec<TableSummaryRow>,  // top 10
}

pub fn summarize(conn: &mut LiveConnection) -> Result<DatabaseSummary>;
```

### 3.8 trend

Time-series: counts/averages grouped by day/week/month/year.

**SQL (generated dynamically):**
```sql
SELECT date_trunc('{interval}', {timestamp_column}) AS period,
       COUNT(*) AS count
FROM {schema}.{table}
GROUP BY period ORDER BY period;

-- With optional value column:
SELECT date_trunc('{interval}', {timestamp_column}) AS period,
       COUNT(*) AS count, AVG({value_column})::numeric(20,4) AS avg_value
FROM {schema}.{table}
GROUP BY period ORDER BY period;
```

```rust
pub enum TrendInterval { Day, Week, Month, Year }

pub struct TrendParams {
    pub table: String,
    pub schema: Option<String>,
    pub timestamp_column: String,
    pub interval: TrendInterval,
    pub value_column: Option<String>,
    pub limit: Option<i64>,
}

pub struct TrendResult {
    pub database_name: String,
    pub table: String,
    pub interval: TrendInterval,
    pub rows: Vec<TrendRow>,
}

pub fn compute_trend(conn: &mut LiveConnection, params: &TrendParams) -> Result<TrendResult>;
```

All user-provided identifiers are validated against `^[a-zA-Z_][a-zA-Z0-9_]*$` to prevent SQL injection.

### 3.9 sample

Preview rows from any table.

**SQL:**
```sql
SELECT * FROM {schema}.{table} LIMIT {limit};
-- With optional ordering:
SELECT * FROM {schema}.{table} ORDER BY {order_column} DESC LIMIT {limit};
```

```rust
pub struct SampleResult {
    pub database_name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows_in_table: i64,  // from pg_stat_user_tables.n_live_tup
    pub rows_returned: usize,
}

pub fn sample_table(conn: &mut LiveConnection, table: &str, schema: Option<&str>,
                    limit: Option<i64>, order_by: Option<&str>) -> Result<SampleResult>;
```

### 3.10 erd

Entity-relationship diagram: PKs and foreign keys. ASCII art + Mermaid/DOT export.

**SQL:**
```sql
-- Tables
SELECT table_name FROM information_schema.tables
WHERE table_schema = '{schema}' AND table_type = 'BASE TABLE';

-- Primary keys (same as schema)
-- Foreign keys
SELECT tc.table_name AS from_table, kcu.column_name AS from_column,
       ccu.table_name AS to_table, ccu.column_name AS to_column, tc.constraint_name
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu
    ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
JOIN information_schema.constraint_column_usage ccu
    ON tc.constraint_name = ccu.constraint_name AND tc.table_schema = ccu.table_schema
WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_schema = '{schema}';

-- Columns per table
SELECT table_name, column_name, data_type, is_nullable
FROM information_schema.columns WHERE table_schema = '{schema}'
ORDER BY table_name, ordinal_position;
```

```rust
pub enum ErdFormat { Ascii, Mermaid, Dot }

pub struct ErdResult {
    pub database_name: String,
    pub schema: String,
    pub tables: Vec<ErdTable>,
    pub foreign_keys: Vec<ForeignKey>,
}

pub fn build_erd(conn: &mut LiveConnection, schema: Option<&str>) -> Result<ErdResult>;
pub fn format_erd_ascii(result: &ErdResult) -> String;
pub fn format_erd_mermaid(result: &ErdResult) -> String;
pub fn format_erd_dot(result: &ErdResult) -> String;
```

---

## 4. Read-Only Enforcement

### 4.1 Session-Level (Server-Side)

On every `ConnectionManager::connect()`:
```sql
SET default_transaction_read_only = on;
```
PostgreSQL rejects INSERT/UPDATE/DELETE/DROP/CREATE/ALTER/TRUNCATE with `cannot execute ... in a read-only transaction`.

### 4.2 Client-Side Parsing

`validate_readonly(sql)` strips comments, gets first keyword. Allowed: `SELECT`, `WITH`, `EXPLAIN`, `SHOW`, `TABLE`. Everything else is rejected before sending to server.

Both layers apply to `query` and `compare` commands. The other commands use hardcoded queries that are inherently read-only.

---

## 5. CLI Integration

### 5.1 Args

```rust
pub struct Cli {
    #[arg(short = 'D', long = "directory", global = true)]
    pub directory: Option<String>,

    #[arg(long = "db", global = true)]
    pub databases: Vec<String>,

    #[arg(long = "all", global = true)]
    pub all_databases: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

pub enum Commands {
    Tui,
    ListDatabases,
    Health,
    Schema { #[arg(long, default_value = "public")] schema: String },
    Query { sql: String },
    Analyze { table: String, #[arg(long, default_value = "public")] schema: String },
    Compare { sql: String },
    Summary,
    Trend {
        table: String,
        #[arg(long)] timestamp: String,
        #[arg(long, default_value = "day")] interval: String,
        #[arg(long)] value: Option<String>,
        #[arg(long, default_value = "public")] schema: String,
        #[arg(long)] limit: Option<i64>,
    },
    Sample {
        table: String,
        #[arg(long, default_value = "20")] limit: i64,
        #[arg(long)] order_by: Option<String>,
        #[arg(long, default_value = "public")] schema: String,
    },
    Erd {
        #[arg(long, default_value = "public")] schema: String,
        #[arg(long, default_value = "ascii")] format: String,
        #[arg(long)] output: Option<String>,
    },
}
```

### 5.2 Connection Helper

```rust
fn establish_connections(
    directory: Option<&str>,
    db_names: &[String],
    all: bool,
) -> Result<ConnectionManager> {
    let path = resolve_config_path_with_base(directory)?;
    let configs = load_databases(&path)?;
    let mut manager = ConnectionManager::new();

    if all {
        for config in &configs { manager.connect(config)?; }
    } else if db_names.is_empty() {
        anyhow::bail!("Specify --db <name> or --all to select databases.");
    } else {
        for name in db_names {
            let config = configs.iter().find(|c| c.name == *name)
                .ok_or_else(|| anyhow::anyhow!("No configured database named '{name}'"))?;
            manager.connect(config)?;
        }
    }
    Ok(manager)
}
```

---

## 6. TUI Integration

### 6.1 New Screens and Menu Items

```rust
pub enum Screen {
    Home, CreateConfig, Connect,
    StoredDatabases, DatabaseHealth,
    Schema, Query, Analyze, Compare, Summary, Trend, Sample, Erd,
}

pub enum MenuItem {
    CreateConfig, Connect, StoredDatabases, DatabaseHealth,
    Schema, Query, Analyze, Compare, Summary, Trend, Sample, Erd,
}
```

Menu items after `Connect` are only shown when at least one database is connected.

### 6.2 Connect Screen

Shows configured databases as a checklist:
```
  Connect to Databases

  [x] production    localhost:5432  db=myapp      Connected
  [ ] staging       staging.ex:5432 db=staging_db
  [ ] analytics     analytics.ex:5432 db=data

  Space toggle  Enter confirm  Esc back  q quit
```

### 6.3 AppState Additions

```rust
pub connection_manager: Arc<Mutex<ConnectionManager>>,
pub connect_selection: Vec<bool>,
pub connect_cursor: usize,
pub input_buffer: String,
pub input_cursor: usize,
pub input_mode: bool,
// Result fields per command
pub schema_result: Option<Vec<SchemaResult>>,
pub query_result: Option<QueryResultSet>,
pub analyze_result: Option<TableProfile>,
pub compare_result: Option<CompareResult>,
pub summary_result: Option<Vec<DatabaseSummary>>,
pub trend_result: Option<TrendResult>,
pub sample_result: Option<SampleResult>,
pub erd_result: Option<ErdResult>,
```

### 6.4 Background Execution

Generalize the health check pattern with a single enum-typed receiver:

```rust
enum BackgroundResult {
    Health(Vec<EnhancedHealthResult>),
    Schema(Result<Vec<SchemaResult>, String>),
    Query(Result<QueryResultSet, String>),
    // ... etc
}

// In run_loop:
let mut background_rx: Option<mpsc::Receiver<BackgroundResult>> = None;
```

### 6.5 File Structure

Split monolithic `ui.rs` and `event.rs` into directories:

```
crates/databasecli-tui/src/ui/
  mod.rs, home.rs, connect.rs, databases.rs, health.rs,
  schema.rs, query.rs, analyze.rs, compare.rs, summary.rs,
  trend.rs, sample.rs, erd.rs

crates/databasecli-tui/src/event/
  mod.rs, home.rs, connect.rs, query.rs, ...
```

---

## 7. New Error Types

Added to `error.rs`:

```rust
AlreadyConnected(String),
NotConnected(String),
NoConnections,
QueryFailed(String),
ReadOnlyViolation(String),
EmptyQuery,
InvalidIdentifier(String),
TableNotFound { schema: String, table: String },
ColumnNotFound { schema: String, table: String, column: String },
InvalidInterval(String),
Postgres(#[from] postgres::Error),
```

---

## 8. Phased Implementation

### Phase 1: Foundation

Connection manager + list_databases + health (enhanced) + query + sample + schema.

1. `connection.rs` -- ConnectionManager, LiveConnection
2. `commands/query.rs` -- validate_readonly, strip_sql_comments, execute_query
3. `commands/mod.rs` -- validate_identifier utility
4. `commands/list_databases.rs`, `commands/health.rs`, `commands/sample.rs`, `commands/schema.rs`
5. Error types in `error.rs`
6. CLI: `--db`/`--all` flags, new Commands variants, `establish_connections`, `run_*` functions
7. TUI: Connect screen, `Arc<Mutex<ConnectionManager>>` in AppState, split ui.rs/event.rs into directories, text input handling, BackgroundResult enum
8. Add `regex` to core Cargo.toml

### Phase 2: Analysis

analyze + summary + erd.

1. `commands/analyze.rs`, `commands/summary.rs`, `commands/erd.rs`
2. CLI: Analyze, Summary, Erd match arms and run functions
3. TUI: Analyze, Summary, Erd screens with input forms

### Phase 3: Multi-Database and Time-Series

compare + trend.

1. `commands/compare.rs`, `commands/trend.rs`
2. CLI: Compare, Trend match arms
3. TUI: Compare screen (SQL input + multi-DB results), Trend screen (form with table/timestamp/interval)

---

## 9. File Changes Per Phase

### Phase 1

**New files:**
- `crates/databasecli-core/src/connection.rs`
- `crates/databasecli-core/src/commands/mod.rs`
- `crates/databasecli-core/src/commands/list_databases.rs`
- `crates/databasecli-core/src/commands/health.rs`
- `crates/databasecli-core/src/commands/query.rs`
- `crates/databasecli-core/src/commands/sample.rs`
- `crates/databasecli-core/src/commands/schema.rs`
- `crates/databasecli-tui/src/ui/` (mod.rs + per-screen files)
- `crates/databasecli-tui/src/event/` (mod.rs + per-screen files)

**Modified files:**
- `crates/databasecli-core/Cargo.toml` -- add `regex`
- `crates/databasecli-core/src/lib.rs` -- add `pub mod connection; pub mod commands;`
- `crates/databasecli-core/src/error.rs` -- add new variants
- `crates/databasecli-cli/src/args.rs` -- `--db`, `--all`, new Commands
- `crates/databasecli-cli/src/main.rs` -- new match arms
- `crates/databasecli-cli/src/run.rs` -- `establish_connections`, new `run_*` functions
- `crates/databasecli-tui/src/lib.rs` -- Arc<Mutex<CM>>, BackgroundResult, generalized receiver
- `crates/databasecli-tui/src/app.rs` -- new Screen/MenuItem/AppAction variants, connection + input + result state

**Deleted (replaced by directories):**
- `crates/databasecli-tui/src/ui.rs`
- `crates/databasecli-tui/src/event.rs`

### Phase 2

**New:** `commands/analyze.rs`, `commands/summary.rs`, `commands/erd.rs`, `ui/analyze.rs`, `ui/summary.rs`, `ui/erd.rs`, `event/analyze.rs`

**Modified:** `commands/mod.rs`, `args.rs`, `main.rs`, `run.rs`, `app.rs`, `lib.rs`, `ui/mod.rs`, `event/mod.rs`

### Phase 3

**New:** `commands/compare.rs`, `commands/trend.rs`, `ui/compare.rs`, `ui/trend.rs`, `event/compare.rs`, `event/trend.rs`

**Modified:** Same set as Phase 2.

### New Dependencies

- `regex` in `databasecli-core` (identifier validation)
- No other new crate dependencies needed.
