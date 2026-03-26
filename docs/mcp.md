# databasecli MCP Server

A read-only MCP (Model Context Protocol) server that gives AI agents secure access to PostgreSQL databases. The server communicates over stdio and exposes 14 tools for database discovery, querying, schema inspection, and analysis.

## Security Model

All database connections enforce read-only access at two layers:

1. **Server-side**: `SET default_transaction_read_only = on` on every connection. PostgreSQL itself rejects INSERT, UPDATE, DELETE, DROP, CREATE, ALTER, TRUNCATE.
2. **Client-side**: SQL validation rejects anything that isn't SELECT, WITH, EXPLAIN, SHOW, or TABLE. Multi-statement queries (containing `;`) are also rejected.

Additionally:
- Statement timeout of 30 seconds prevents runaway queries.
- Database passwords are never exposed to the agent. Databases are referenced by name only.
- The `suggest_migration` tool analyzes schema and returns context, but **never executes DDL**.

## Installation

```bash
# From the project root
cargo build -p databasecli-mcp --release

# The binary is at target/release/databasecli-mcp
```

## Configuration

### Database Config File

The server reads database connections from an INI file at `~/.databasecli/databases.ini` (or `<directory>/.databasecli/databases.ini` when using `-D`):

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
databasecli-mcp                    # uses ~/.databasecli/databases.ini
databasecli-mcp -D /project/path   # uses /project/path/.databasecli/databases.ini
```

The server reads JSON-RPC messages from stdin and writes responses to stdout. Logs go to stderr.

## Tools Reference

### Connection Management

#### list_configured_databases

Discover what databases are available in the config file. Call this first.

- **Parameters**: none
- **Returns**: JSON array of `{name, host, port, dbname, user}` (passwords excluded)

#### connect_databases

Establish persistent read-only connections to one or more databases.

- **Parameters**: `{"names": ["production", "staging"]}`
- **Returns**: `{"connected": ["production"], "errors": [{"name": "bad", "error": "..."}]}`

Connections persist for the lifetime of the MCP session. All subsequent tools operate on connected databases.

#### disconnect_databases

Drop connections. Pass empty array to disconnect all.

- **Parameters**: `{"names": ["production"]}` or `{"names": []}`
- **Returns**: `{"disconnected": [...], "still_connected": [...]}`

#### list_connected_databases

Show currently active connections.

- **Parameters**: none
- **Returns**: JSON array of `{name, host, port, dbname, user}`

### Querying

#### query

Execute a read-only SQL query on a connected database.

- **Parameters**: `{"sql": "SELECT * FROM users LIMIT 10", "database": "production"}`
  - `database` is optional; omit to use the first connected database
- **Returns**: `{"database": "production", "columns": [...], "rows": [[...]], "row_count": 10, "execution_time_ms": 42}`
- **Allowed statements**: SELECT, WITH, EXPLAIN, SHOW, TABLE
- **Blocked**: INSERT, UPDATE, DELETE, DROP, CREATE, ALTER, TRUNCATE, multi-statement (`;`)

#### compare

Run the same query across ALL connected databases. Useful for comparing production vs staging.

- **Parameters**: `{"sql": "SELECT count(*) FROM users"}`
- **Returns**: `{"query": "...", "results": [{per-database result}], "errors": [{per-database errors}]}`
- **Requires**: 2+ connected databases for meaningful comparison

### Schema Inspection

#### schema

Get full schema: tables, columns, data types, primary keys, row counts, sizes.

- **Parameters**: `{"schema_name": "public", "database": "production"}`
  - Both optional. Omit `database` to get schema for all connected databases.
- **Returns**: `{"schemas": [{database, tables: [{name, columns, primary_key_columns, row_count, total_size}]}], "errors": [...]}`

#### sample

Preview rows from a table.

- **Parameters**: `{"table": "users", "database": "production", "limit": 10, "order_by": "created_at"}`
  - `schema_name` defaults to `"public"`, `limit` defaults to `20`, `order_by` is optional (DESC)
- **Returns**: `{columns, rows, total_rows_in_table, rows_returned}`

#### erd

Entity-relationship diagram showing tables, columns, PKs, and foreign keys.

- **Parameters**: `{"database": "production", "schema_name": "public"}`
- **Returns**: `{"mermaid": "erDiagram\n...", "tables": [...], "foreign_keys": [...]}`
  - `mermaid` field contains renderable Mermaid syntax
  - `tables` and `foreign_keys` fields contain structured data for programmatic use

### Analysis

#### analyze

Profile a table: per-column null counts, distinct values, min/max/avg, top 10 most frequent values.

- **Parameters**: `{"table": "orders", "database": "production"}`
- **Returns**: Column profiles with `{name, data_type, null_count, null_pct, distinct_count, min_value, max_value, avg_value, top_values}`

#### summary

High-level database overview.

- **Parameters**: `{"database": "production"}` (optional; omit for all connected)
- **Returns**: `{"summaries": [{database, table_count, total_rows, total_size, index_count, largest_tables}], "errors": [...]}`

#### trend

Time-series analysis: group rows by a timestamp column at day/week/month/year intervals.

- **Parameters**: `{"table": "orders", "database": "production", "timestamp_column": "created_at", "interval": "day", "value_column": "amount", "limit": 30}`
  - `interval`: `day`, `week`, `month`, or `year` (default: `day`)
  - `value_column`: optional numeric column to compute AVG per period
  - `limit`: max periods to return
- **Returns**: `{rows: [{period, count, avg_value}]}`

#### enhanced_health

Health check for all connected databases: PostgreSQL version, database size, uptime, response time.

- **Parameters**: none
- **Returns**: JSON array of `{name, status, pg_version, db_size, uptime, response_time_ms}`

### Migration Planning

#### suggest_migration

Gather schema context for planning DDL changes. This tool **never executes DDL**. It returns the current schema and foreign key relationships so you can generate migration SQL for the user to review.

- **Parameters**: `{"database": "production", "description": "Add soft delete column to users table", "schema_name": "public"}`
- **Returns**: `{"current_schema": {...}, "foreign_keys": [...], "description": "...", "note": "..."}`

## Typical Agent Workflow

### 1. Discover and Connect

```
Agent: Call list_configured_databases
Agent: Call connect_databases with {"names": ["production"]}
```

### 2. Explore Schema

```
Agent: Call schema to see all tables
Agent: Call erd to understand relationships
Agent: Call sample on a specific table to see data shape
```

### 3. Answer User Questions

```
User: "Show me all tickets for event Ballad Beast"

Agent: Call schema to find relevant tables (events, tickets)
Agent: Call sample on events to verify column names
Agent: Call query with {"sql": "SELECT t.* FROM tickets t JOIN events e ON t.event_id = e.id WHERE e.name = 'Ballad Beast'"}
Agent: Present results to user
```

### 4. Analyze Data Quality

```
Agent: Call analyze on a table to see null rates, cardinality
Agent: Call trend to see how data grows over time
Agent: Call summary for an overview of database health
```

### 5. Plan Schema Changes

```
Agent: Call suggest_migration with description of desired changes
Agent: Use returned schema context to generate ALTER/CREATE SQL
Agent: Present migration SQL to user for review (never auto-executed)
```

## Architecture

```
AI Agent (Claude, etc.)
    |
    | stdio (JSON-RPC)
    |
databasecli-mcp (async, tokio)
    |
    | Arc<Mutex<ConnectionManager>>
    | tokio::task::spawn_blocking
    |
databasecli-core (sync, postgres crate)
    |
    | SET default_transaction_read_only = on
    | SET statement_timeout = '30s'
    |
PostgreSQL
```

The MCP server is async (tokio + rmcp). The core library is synchronous (postgres crate). The bridge uses `spawn_blocking` to run sync database operations on tokio's blocking thread pool without blocking the event loop. A `Mutex<ConnectionManager>` with poison-recovery protects shared state.
