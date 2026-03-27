# databasecli MCP Server

A read-only MCP (Model Context Protocol) server that gives AI agents secure access to PostgreSQL databases. The server communicates over stdio and exposes 14 tools for database discovery, querying, schema inspection, and analysis.

## Security Model

All database connections enforce read-only access at two layers:

1. **Server-side**: `SET default_transaction_read_only = on` on every connection. PostgreSQL itself rejects INSERT, UPDATE, DELETE, DROP, CREATE, ALTER, TRUNCATE.
2. **Client-side**: SQL validation rejects anything that isn't SELECT, WITH, EXPLAIN, SHOW, or TABLE. Multi-statement queries (containing `;` outside string literals) are also rejected.

Additionally:
- Statement timeout of 30 seconds prevents runaway queries.
- Database passwords are never exposed to the agent. Databases are referenced by name only.
- The `suggest_migration` tool analyzes schema and returns context, but **never executes DDL**.
- TLS is used for all connections, but certificate verification is currently disabled (`danger_accept_invalid_certs`). This means connections are encrypted but not verified against a CA. Suitable for internal/dev databases; not recommended for connections over untrusted networks without additional network-level security.

## Quick Start

```bash
# 1. Build the server
cargo build -p databasecli-mcp --release

# 2. Create a config file with a template
./target/release/databasecli-mcp --init

# 3. Edit the config file with your database connections
#    Default location: ~/.databasecli/databases.ini
#    With -D flag: <directory>/.databasecli/databases.ini

# 4. Configure your MCP client (see sections below) and start using it
```

The `--init` flag creates the config directory and a template INI file. If the file already exists, it tells you and does not overwrite it. You can also use `--init` with `-D` to create the config in a specific project directory:

```bash
./target/release/databasecli-mcp -D /path/to/project --init
```

## Configuration

### Database Config File

The server reads database connections from an INI file. Each section defines one database:

```ini
[production]
host = db.example.com
port = 5432
user = readonly_user
password = secret
dbname = myapp

[staging]
host = staging-db.example.com
port = 5432
user = readonly_user
password = secret
dbname = myapp_staging
```

Each section name becomes the database identifier that the agent uses to connect.

**Passwords** are stored in plaintext in the INI file. Ensure the file has restricted permissions (`chmod 600`). Future versions may support OS keychains or environment variable references.

### Config Path Resolution

The config file path is resolved in this priority order:

1. **`DATABASECLI_CONFIG_PATH` env var** — if set, uses this exact path (highest priority)
2. **`-D <directory>` flag** — uses `<directory>/.databasecli/databases.ini`
3. **Default (release)** — `~/.databasecli/databases.ini`
4. **Default (debug build)** — `<exe-directory>/databases-dev.ini`

If the config file does not exist or cannot be parsed, the server starts with zero configured databases and logs a warning to stderr.

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "databasecli": {
      "command": "/path/to/databasecli-mcp",
      "args": ["-D", "/path/to/project"]
    }
  }
}
```

### Claude Code

Add to `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "databasecli": {
      "command": "/path/to/databasecli-mcp",
      "args": ["-D", "."]
    }
  }
}
```

### Other MCP Clients

Any MCP client that supports stdio transport can use the server:

```bash
databasecli-mcp                    # uses default config path
databasecli-mcp -D /project/path   # uses /project/path/.databasecli/databases.ini
```

The server reads JSON-RPC messages from stdin and writes responses to stdout. Logs go to stderr. The session ends when stdin closes (e.g., the MCP client exits or the pipe is closed).

## Tools Reference

### Connection Management

#### list_configured_databases

Discover what databases are available in the config file. Call this first before connecting.

- **Parameters**: none
- **Returns**: JSON array of `{name, host, port, dbname, user}` (passwords are never included)
- **Requires connection**: no

#### connect_databases

Establish persistent read-only connections to one or more databases by name.

- **Parameters**: `{"names": ["production", "staging"]}`
- **Returns**: `{"connected": ["production"], "errors": [{"name": "bad", "error": "..."}]}`
- **Requires connection**: no (this creates connections)

Connections persist for the lifetime of the MCP session. All subsequent tools operate on connected databases. If a connection fails for one name, others still connect — check the `errors` array.

#### disconnect_databases

Drop connections. Pass an empty array to disconnect all.

- **Parameters**: `{"names": ["production"]}` or `{"names": []}` (disconnect all)
- **Returns**: `{"disconnected": [...], "still_connected": [...]}`

When passing an empty array, `disconnected` lists all databases that were connected before the call.

#### list_connected_databases

Show currently active connections with their details.

- **Parameters**: none
- **Returns**: JSON array of `{name, host, port, dbname, user}`
- **Requires connection**: yes (returns empty array if none connected)

### Querying

#### query

Execute a read-only SQL query on a connected database.

- **Parameters**: `{"sql": "SELECT * FROM users LIMIT 10", "database": "production"}`
  - `database` is optional; omit to query the first connected database
- **Returns**: `{"database": "production", "columns": ["id", "name"], "rows": [["1", "alice"]], "row_count": 1, "execution_time_ms": 42}`
- **Allowed statements**: SELECT, WITH, EXPLAIN, SHOW, TABLE
- **Blocked**: INSERT, UPDATE, DELETE, DROP, CREATE, ALTER, TRUNCATE, multi-statement (containing `;`)
- **Requires connection**: yes

All values in `rows` are returned as strings regardless of the original PostgreSQL type. See [Supported Data Types](#supported-data-types) for details.

**Important**: There is no built-in row limit on `query`. For large tables, always include a `LIMIT` clause in your SQL to avoid excessive memory usage and slow responses.

#### compare

Run the same query across ALL connected databases and return results side by side.

- **Parameters**: `{"sql": "SELECT count(*) FROM users"}`
- **Returns**: `{"query": "...", "results": [{per-database result}], "errors": [{per-database errors}]}`
- **Requires connection**: yes (works with 1 database but is most useful with 2+)

If a query fails on one database, other databases still return results. Check both `results` and `errors` arrays.

### Schema Inspection

#### schema

Get full schema: tables, columns, data types, primary keys, row counts, sizes.

- **Parameters**: `{"schema_name": "public", "database": "production"}`
  - `schema_name` defaults to `"public"` if omitted
  - `database` is optional; omit to get schema for all connected databases
- **Returns**: `{"schemas": [{database, tables: [{schema, name, row_count, total_size, columns, primary_key_columns}]}], "errors": [...]}`
- **Requires connection**: yes

If one database fails, others still return results. Check both `schemas` and `errors` arrays.

#### sample

Preview rows from a table.

- **Parameters**:
  - `table` (required): table name
  - `database` (required): database name
  - `schema_name`: defaults to `"public"`
  - `limit`: max rows to return, defaults to `20`
  - `order_by`: optional column name to ORDER BY DESC (useful for seeing newest records)
- **Returns**: `{database, table, columns, rows, total_rows_in_table, rows_returned}`
- **Requires connection**: yes

`total_rows_in_table` is an approximate count from PostgreSQL statistics (`pg_stat_user_tables.n_live_tup`), not an exact `COUNT(*)`. It may be stale if `ANALYZE` hasn't run recently.

#### erd

Entity-relationship diagram showing tables, columns, primary keys, and foreign keys.

- **Parameters**:
  - `database` (required): database name
  - `schema_name`: defaults to `"public"`
- **Returns**: `{"database": "...", "schema": "...", "mermaid": "erDiagram\n...", "tables": [...], "foreign_keys": [...]}`
  - `mermaid`: renderable Mermaid syntax for the ERD
  - `tables`: structured array with columns and primary keys per table
  - `foreign_keys`: array of `{from_table, from_column, to_table, to_column, constraint_name}`
- **Requires connection**: yes

### Analysis

#### analyze

Profile a table: per-column null counts, distinct values, min/max/avg for numeric columns, top 10 most frequent values.

- **Parameters**:
  - `table` (required): table name
  - `database` (required): database name
  - `schema_name`: defaults to `"public"`
- **Returns**: `{database, schema, table, total_rows, columns: [{name, data_type, total_rows, non_null_count, null_count, null_pct, distinct_count, min_value, max_value, avg_value, top_values}]}`
- **Requires connection**: yes

This tool runs multiple queries per column (null stats, min/max, top values). For tables with many columns, execution may take several seconds.

#### summary

High-level database overview: size, table count, row count, indexes, largest tables.

- **Parameters**:
  - `database`: optional; omit to summarize all connected databases
- **Returns**: `{"summaries": [{database, table_count, total_rows, total_size, index_count, largest_tables}], "errors": [...]}`
- **Requires connection**: yes

If one database fails, others still return results. Check both `summaries` and `errors` arrays.

#### trend

Time-series analysis: group rows by a timestamp column at day/week/month/year intervals.

- **Parameters**:
  - `table` (required): table name
  - `database` (required): database name
  - `timestamp_column` (required): name of the timestamp/date column
  - `interval`: `"day"`, `"week"`, `"month"`, or `"year"` (default: `"day"`)
  - `value_column`: optional numeric column to compute AVG per period
  - `schema_name`: defaults to `"public"`
  - `limit`: max number of periods to return
- **Returns**: `{database, table, interval, rows: [{period, count, avg_value}]}`
- **Requires connection**: yes

Rows with NULL in the timestamp column are excluded from the trend.

#### enhanced_health

Health check for all connected databases: PostgreSQL version, database size, uptime, response time.

- **Parameters**: none
- **Returns**: JSON array of `{name, host, port, dbname, status, response_time_ms, pg_version, db_size, uptime, error}`
  - `status` is `"connected"` or `"failed"`
  - `error` is null when status is connected
- **Requires connection**: yes

### Migration Planning

#### suggest_migration

Gather schema context for planning DDL changes. This tool **never executes DDL**. It returns the current schema and foreign key relationships so the agent can generate migration SQL for the user to review.

- **Parameters**:
  - `database` (required): database to analyze
  - `description` (required): natural language description of desired schema changes
  - `schema_name`: defaults to `"public"`
- **Returns**: `{"current_schema": {...}, "foreign_keys": [...], "description": "...", "note": "This is an analysis tool..."}`
- **Requires connection**: yes

The agent should use the returned schema context to generate appropriate ALTER/CREATE/DROP SQL, then present it to the user for manual review and execution. The generated SQL is never executed through this tool.

## Supported Data Types

All query results (`query`, `compare`, `sample`) return values as strings. The following PostgreSQL types are natively supported with proper formatting:

| PostgreSQL Type | Output Format | Example |
|-----------------|---------------|---------|
| `boolean` | `true` / `false` | `true` |
| `smallint`, `integer`, `bigint` | Numeric string | `42` |
| `real`, `double precision` | Decimal string | `3.14` |
| `uuid` | Hyphenated UUID | `34bf888f-1aad-4a62-89b4-af880cf5e236` |
| `timestamp with time zone` | RFC 3339 | `2026-03-12T10:42:45.241329+00:00` |
| `timestamp without time zone` | ISO 8601 | `2026-03-12 10:42:45.241329` |
| `date` | ISO 8601 | `2026-03-12` |
| `time` | ISO 8601 | `14:30:00` |
| `json`, `jsonb` | JSON string | `{"key": "value"}` |
| `text`, `varchar`, `char` | Plain string | `hello world` |

Other types (e.g., `inet`, `bytea`, `interval`, `numeric`, arrays) fall back to PostgreSQL's text representation when possible. If a type cannot be converted, the value appears as `(unsupported type)`.

NULL values for any type appear as the string `NULL`.

## Error Handling

Tools return errors in two ways:

### Application Errors (tool executed, but something went wrong)

Returned as `{"content": [{"type": "text", "text": "Error: ..."}], "isError": false}`. These include:
- Database not connected (`Error: not connected to 'production'`)
- SQL validation failure (`Error: read-only violation: statement begins with 'DELETE'`)
- Table not found (`Error: table not found: public.nonexistent`)
- PostgreSQL query errors (`Error: database error: column "foo" does not exist`)
- Invalid identifier (`Error: invalid identifier 'drop;--'`)

The agent should read the error message and adjust its approach (fix the SQL, connect first, check table name, etc.).

### Protocol Errors (something went wrong internally)

Returned as JSON-RPC error responses. These include mutex poisoning, task panics, or transport failures. These are rare and indicate a server-level problem. The agent should report the error to the user.

### Multi-Database Error Collection

Tools that operate on multiple databases (`schema`, `summary`, `compare`) collect per-database errors alongside successes. If one database fails, results from other databases are still returned. Always check the `errors` array in the response.

## Typical Agent Workflow

### 1. Discover and Connect

```
Agent: Call list_configured_databases
       → See available databases and their names
Agent: Call connect_databases with {"names": ["production"]}
       → Check "connected" and "errors" in response
```

### 2. Explore Schema

```
Agent: Call schema to see all tables, columns, types, PKs
Agent: Call erd to understand foreign key relationships
Agent: Call sample on a specific table to see actual data shape
```

### 3. Answer User Questions

```
User: "Show me all tickets for event Ballad Beast"

Agent: Call schema to find relevant tables (events, tickets)
Agent: Call sample on events table to verify column names
Agent: Call query with a JOIN query filtering by event name
Agent: Present results to user with summary
```

### 4. Analyze Data Quality

```
Agent: Call analyze on a table to see null rates, cardinality, value distribution
Agent: Call trend to see how data grows over time
Agent: Call summary for an overview of database size and structure
Agent: Call enhanced_health to check PostgreSQL version and uptime
```

### 5. Compare Environments

```
Agent: Call connect_databases with {"names": ["production", "staging"]}
Agent: Call compare with {"sql": "SELECT count(*) FROM users"}
       → See row counts side by side from both databases
```

### 6. Plan Schema Changes

```
Agent: Call suggest_migration with description of desired changes
       → Receives current schema context and foreign keys
Agent: Generate ALTER/CREATE SQL based on the schema context
Agent: Present migration SQL to user for manual review and execution
```

## Known Limitations

### No Auto-Reconnect

If a database connection drops during a session (network timeout, server restart, pgBouncer idle timeout), subsequent tool calls to that database will fail. The agent must explicitly call `disconnect_databases` then `connect_databases` to re-establish the connection. There is no automatic reconnection or liveness check.

### No Query Result Size Limit

The `query` and `compare` tools have no built-in row limit. A query like `SELECT * FROM large_table` without a `LIMIT` clause could cause the server to allocate large amounts of memory serializing millions of rows to JSON. Always include `LIMIT` in queries when the result size is unknown. The `sample` tool defaults to 20 rows.

### Statement Timeout

All queries have a 30-second timeout (`SET statement_timeout = '30s'`). Queries exceeding this limit are cancelled by PostgreSQL. This protects against runaway queries but means complex analytical queries on large tables may need to be broken into smaller operations.

### Single-Threaded Query Execution

All tool calls share a single `Mutex<ConnectionManager>`. If the agent sends multiple tool calls concurrently (MCP supports pipelining), they execute sequentially — the second call blocks until the first finishes. There is no deadlock risk, but concurrent calls will not run in parallel.

### TLS Certificate Verification

TLS certificate verification is disabled for all connections. Connections are encrypted but the server's identity is not verified against a certificate authority. This is suitable for internal networks and development environments.

### Approximate Row Counts

The `schema` and `sample` tools report row counts from `pg_stat_user_tables.n_live_tup`, which is updated by PostgreSQL's autovacuum/autoanalyze. On tables that haven't been analyzed recently, these counts may be significantly inaccurate.

## Debugging

### Enable Debug Logging

The server writes logs to stderr. Control log verbosity with the `RUST_LOG` environment variable:

```bash
# Default: INFO level
databasecli-mcp -D /project/path

# Debug level (shows all tool calls and timing)
RUST_LOG=debug databasecli-mcp -D /project/path

# Trace level (shows JSON-RPC messages)
RUST_LOG=trace databasecli-mcp -D /project/path
```

In Claude Desktop config:

```json
{
  "mcpServers": {
    "databasecli": {
      "command": "databasecli-mcp",
      "args": ["-D", "/path/to/project"],
      "env": {
        "RUST_LOG": "debug"
      }
    }
  }
}
```

### Troubleshooting

| Problem | Cause | Solution |
|---------|-------|----------|
| Server starts but no tools appear | MCP client didn't send `initialized` notification | Check client configuration and protocol version |
| `"errors": [{"name": "prod", "error": "connection failed: ..."}]` | Database unreachable, wrong credentials, or firewall | Verify host/port/user/password in the INI file, test with `psql` |
| `"Error: not connected to 'production'"` | Tool called before `connect_databases` | Call `connect_databases` first |
| `"Error: read-only violation: ..."` | SQL starts with a disallowed keyword | Only SELECT, WITH, EXPLAIN, SHOW, TABLE are allowed |
| `"Error: multi-statement queries (containing ';') are not allowed"` | SQL contains a semicolon outside a string literal | Remove the semicolon, send one statement at a time |
| `"Error: canceling statement due to statement timeout"` | Query exceeded 30-second timeout | Simplify the query, add WHERE clauses, or use LIMIT |
| `(unsupported type)` in query results | Column type not handled by the type converter | Cast the column in SQL: `SELECT col::text FROM ...` |
| Empty `list_configured_databases` response | Config file not found or parse error | Check path resolution, look for warnings in stderr |
| Tool call hangs | Long-running query holding the mutex | Wait up to 30s for statement timeout, or restart the server |
| Connection errors after idle period | Database server closed idle connection | Call `disconnect_databases` then `connect_databases` to reconnect |

## How It Works: Step-by-Step Flow

This section explains what happens end-to-end when you ask an AI agent to query a database.

### Starting a Session

When you open Claude Desktop (or any MCP client), it launches the `databasecli-mcp` process:

```
┌──────────────────┐         ┌──────────────────────┐
│  Claude Desktop  │         │   databasecli-mcp    │
│   (MCP Client)   │         │    (MCP Server)      │
└────────┬─────────┘         └──────────┬───────────┘
         │                              │
         │  1. spawn process            │
         │  ─────────────────────────>  │  reads databases.ini
         │                              │  loads 2 configs (prod, staging)
         │                              │  creates empty ConnectionManager
         │  2. initialize request       │
         │  ─────────────────────────>  │
         │                              │
         │  3. server info + 14 tools   │
         │  <─────────────────────────  │
         │                              │
         │  4. initialized notification │
         │  ─────────────────────────>  │
         │                              │
         │       Session is ready       │
         │                              │
```

At this point the server is running, knows about configured databases, but has no active connections yet.

### Example: "Show me the 5 most recent tickets"

Here's what happens step-by-step when you type this into Claude:

```
┌──────┐      ┌──────────────┐      ┌─────────────────┐      ┌────────────┐
│ You  │      │    Claude    │      │ databasecli-mcp │      │ PostgreSQL │
└──┬───┘      └──────┬───────┘      └────────┬────────┘      └─────┬──────┘
   │                 │                        │                     │
   │ "Show me the   │                        │                     │
   │  5 most recent │                        │                     │
   │  tickets"      │                        │                     │
   │ ──────────────>│                        │                     │
   │                │                        │                     │
```

**Step 1: Claude discovers available databases**

Claude doesn't know what databases exist yet, so it calls `list_configured_databases`:

```
   │                │  tools/call:             │                     │
   │                │  list_configured_databases                     │
   │                │ ──────────────────────>  │                     │
   │                │                          │  reads from memory  │
   │                │                          │  (no DB connection) │
   │                │  [{name:"ticketing",     │                     │
   │                │    host:"10.0.16.17",    │                     │
   │                │    port:5432, ...}]      │                     │
   │                │ <──────────────────────  │                     │
   │                │                          │                     │
```

This is instant — no database connection needed. The server just reads from the configs loaded at startup.

**Step 2: Claude connects to the database**

```
   │                │  tools/call:             │                     │
   │                │  connect_databases       │                     │
   │                │  {names:["ticketing"]}   │                     │
   │                │ ──────────────────────>  │                     │
   │                │                          │                     │
   │                │               ┌──────────┴──────────┐         │
   │                │               │  spawn_blocking     │         │
   │                │               │  lock mutex         │         │
   │                │               │  TLS handshake      │ ──────> │
   │                │               │                     │ <────── │
   │                │               │  SET read_only = on │ ──────> │
   │                │               │  SET timeout = 30s  │ ──────> │
   │                │               │  store connection   │         │
   │                │               │  unlock mutex       │         │
   │                │               └──────────┬──────────┘         │
   │                │                          │                     │
   │                │  {connected:["ticketing"],│                    │
   │                │   errors:[]}             │                     │
   │                │ <──────────────────────  │                     │
   │                │                          │                     │
```

The connection is now stored in the `ConnectionManager` and persists across all future tool calls.

**Step 3: Claude explores the schema to find the right table**

```
   │                │  tools/call: schema      │                     │
   │                │  {database:"ticketing"}  │                     │
   │                │ ──────────────────────>  │                     │
   │                │               ┌──────────┴──────────┐         │
   │                │               │  lock mutex         │         │
   │                │               │  query pg_stat_*    │ ──────> │
   │                │               │  query info_schema  │ ──────> │
   │                │               │  query constraints  │ ──────> │
   │                │               │  unlock mutex       │         │
   │                │               └──────────┬──────────┘         │
   │                │                          │                     │
   │                │  {schemas:[{tables:[     │                     │
   │                │    {name:"customer_tickets",                   │
   │                │     columns:[...],       │                     │
   │                │     row_count:23}]}]}    │                     │
   │                │ <──────────────────────  │                     │
   │                │                          │                     │
```

Claude now sees all tables, columns, and types. It identifies `customer_tickets` as the relevant table and `created_at` as the timestamp column.

**Step 4: Claude builds and executes the query**

```
   │                │  tools/call: query       │                     │
   │                │  {sql: "SELECT           │                     │
   │                │    ticket_number, title,  │                     │
   │                │    status, created_at     │                     │
   │                │    FROM customer_tickets  │                     │
   │                │    ORDER BY created_at    │                     │
   │                │    DESC LIMIT 5",        │                     │
   │                │   database:"ticketing"}  │                     │
   │                │ ──────────────────────>  │                     │
   │                │               ┌──────────┴──────────┐         │
   │                │               │  validate_readonly  │         │
   │                │               │   ✓ starts with     │         │
   │                │               │     SELECT          │         │
   │                │               │   ✓ no semicolons   │         │
   │                │               │  lock mutex         │         │
   │                │               │  execute SQL        │ ──────> │
   │                │               │  receive rows       │ <────── │
   │                │               │  convert types      │         │
   │                │               │   uuid → string     │         │
   │                │               │   timestamptz →     │         │
   │                │               │     RFC 3339        │         │
   │                │               │  unlock mutex       │         │
   │                │               └──────────┬──────────┘         │
   │                │                          │                     │
   │                │  {columns:[...],         │                     │
   │                │   rows:[[27,"jjjj",      │                     │
   │                │     "OPEN","2026-03..."], │                     │
   │                │     ...],                │                     │
   │                │   row_count:5,           │                     │
   │                │   execution_time_ms:42}  │                     │
   │                │ <──────────────────────  │                     │
   │                │                          │                     │
```

**Step 5: Claude presents the results to you**

```
   │                │                          │                     │
   │  "Here are     │                          │                     │
   │  the 5 most    │                          │                     │
   │  recent        │                          │                     │
   │  tickets:      │                          │                     │
   │  #27 jjjj ...  │                          │                     │
   │  #25 arman..." │                          │                     │
   │ <──────────────│                          │                     │
   │                │                          │                     │
```

### Security Checkpoints in the Flow

Every query passes through multiple safety layers:

```
User's question
    │
    ▼
Claude builds SQL ──── "SELECT ... FROM ... LIMIT 5"
    │
    ▼
MCP tool call ──────── JSON-RPC over stdio
    │
    ▼
┌─ validate_readonly ─────────────────────────────┐
│  ✓ First keyword is SELECT/WITH/EXPLAIN/SHOW?   │
│  ✓ No semicolons outside string literals?        │
│  ✗ Rejects: INSERT, UPDATE, DELETE, DROP, etc.  │
└─────────────────────────────────────────────────┘
    │ passed
    ▼
┌─ PostgreSQL session settings ───────────────────┐
│  SET default_transaction_read_only = on          │
│  SET statement_timeout = '30s'                   │
│                                                  │
│  Even if validation missed something,            │
│  PostgreSQL itself blocks write operations.      │
└─────────────────────────────────────────────────┘
    │ passed
    ▼
Query executes, results returned as JSON
```

### What Happens If Something Goes Wrong

```
User: "Delete all old tickets"
    │
    ▼
Claude builds: "DELETE FROM customer_tickets WHERE created_at < '2025-01-01'"
    │
    ▼
validate_readonly ──── ✗ REJECTED
    │                   "read-only violation: statement begins with 'DELETE'"
    ▼
Claude receives error, tells user:
    "I can't delete data — this is a read-only connection.
     I can help you write the DELETE query for you to run manually."
```

```
User: "Show me everything in the huge_logs table"
    │
    ▼
Claude builds: "SELECT * FROM huge_logs"  (no LIMIT — 50 million rows)
    │
    ▼
validate_readonly ──── ✓ passed (it's a SELECT)
    │
    ▼
PostgreSQL starts executing...
    │
    ▼ after 30 seconds
    │
statement_timeout ──── ✗ CANCELLED
    "canceling statement due to statement timeout"
    │
    ▼
Claude receives timeout error, retries with:
    "SELECT * FROM huge_logs ORDER BY created_at DESC LIMIT 100"
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Your Computer                          │
│                                                             │
│  ┌───────────────────┐       ┌───────────────────────────┐  │
│  │   Claude Desktop  │ stdio │    databasecli-mcp        │  │
│  │   or Claude Code  │◄─────►│                           │  │
│  │   (MCP Client)    │ JSON- │  ┌─────────────────────┐  │  │
│  │                   │ RPC   │  │ rmcp (async, tokio)  │  │  │
│  │  You talk to      │       │  │                     │  │  │
│  │  Claude here      │       │  │  Arc<Mutex<         │  │  │
│  └───────────────────┘       │  │   ConnectionManager │  │  │
│                              │  │  >>                 │  │  │
│                              │  │       │             │  │  │
│                              │  │  spawn_blocking     │  │  │
│                              │  │       │             │  │  │
│                              │  │  ┌────▼──────────┐  │  │  │
│                              │  │  │databasecli-   │  │  │  │
│                              │  │  │core (sync)    │  │  │  │
│                              │  │  │               │  │  │  │
│                              │  │  │ read_only=on  │  │  │  │
│                              │  │  │ timeout=30s   │  │  │  │
│                              │  │  └───────┬───────┘  │  │  │
│                              │  └──────────┼──────────┘  │  │
│                              └─────────────┼─────────────┘  │
│                                            │ TLS            │
└────────────────────────────────────────────┼────────────────┘
                                             │
                                    ┌────────▼────────┐
                                    │   PostgreSQL    │
                                    │   Database      │
                                    │                 │
                                    │  Enforces:      │
                                    │  read-only txn  │
                                    │  stmt timeout   │
                                    └─────────────────┘
```

The MCP server is async (tokio + rmcp). The core library is synchronous (postgres crate). The bridge uses `spawn_blocking` to run sync database operations on tokio's blocking thread pool without blocking the event loop. A `Mutex<ConnectionManager>` with poison-recovery protects shared state.

### Session Lifecycle

1. MCP client starts the `databasecli-mcp` process
2. Server reads `databases.ini`, loads configs into memory
3. Client sends `initialize` request — server responds with capabilities and 14 tools
4. Client sends `notifications/initialized` — session is ready
5. Client calls tools via `tools/call` — connections and state persist across calls
6. Session ends when stdin closes (client exits or pipe closes)
7. Server process exits — all database connections are dropped via Rust's RAII (no manual cleanup needed)
