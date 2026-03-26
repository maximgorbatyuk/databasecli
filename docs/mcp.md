# Design: MCP Server for AI Agent Integration

## 1. Overview

databasecli becomes an MCP (Model Context Protocol) server, allowing AI agents (Claude, etc.) to interact with PostgreSQL databases through a secure, read-only gateway. The server communicates via stdio JSON-RPC and exposes databasecli's existing commands as MCP tools.

**Key properties:**
- **Read-only**: All connections use `SET default_transaction_read_only = on` + client-side SQL validation
- **Stateful sessions**: Agent connects to databases, connections persist across tool calls
- **Passwords hidden**: Agent references databases by name, never sees credentials
- **Analyze-only DDL**: Agent can inspect schema and suggest migrations, never execute DDL

## 2. Architecture

New crate `databasecli-mcp` — separate binary, shares `databasecli-core`.

```
crates/databasecli-mcp/
  Cargo.toml
  src/
    main.rs           — clap args, tokio runtime, .serve(stdio()).await
    state.rs          — McpSessionState: configs + Arc<Mutex<ConnectionManager>>
    server.rs         — DatabaseCliServer with #[tool_router] and #[tool_handler]
    tools/
      mod.rs
      connection.rs   — list_configured, connect, disconnect, list_connected
      query.rs        — query, compare
      schema.rs       — schema, erd, sample
      analysis.rs     — analyze, summary, trend, health
      migration.rs    — suggest_migration (analyze-only)
    json_convert.rs   — core structs → serde_json::Value
```

**Dependencies:**
```toml
rmcp = { version = "1.2", features = ["server", "macros", "schemars", "transport-io"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "1"
clap = { version = "4", features = ["derive"] }
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
databasecli-core = { path = "../databasecli-core" }
```

## 3. rmcp API Pattern

```rust
// Server struct with tool router
#[derive(Debug, Clone)]
pub struct DatabaseCliServer {
    tool_router: ToolRouter<Self>,
    state: Arc<McpSessionState>,
}

// Tool definitions
#[tool_router]
impl DatabaseCliServer {
    pub fn new(state: McpSessionState) -> Self {
        Self {
            tool_router: Self::tool_router(),
            state: Arc::new(state),
        }
    }

    #[tool(description = "List all configured databases...")]
    async fn list_configured_databases(&self) -> Result<CallToolResult, McpError> {
        // return JSON
        Ok(CallToolResult::success(vec![Content::text(json_string)]))
    }

    #[tool(description = "Execute read-only SQL...")]
    async fn query(
        &self,
        Parameters(params): Parameters<QueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let mgr = Arc::clone(&self.state.manager);
        let result = tokio::task::spawn_blocking(move || {
            let mut m = mgr.lock().unwrap();
            execute_query(m.get_mut(&db).unwrap(), &params.sql)
        }).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;
        // convert result to JSON
    }
}

// ServerHandler registration
#[tool_handler]
impl ServerHandler for DatabaseCliServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("databasecli: read-only PostgreSQL gateway...".into()),
        }
    }
}

// Entrypoint
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    let state = McpSessionState::new(directory)?;
    let service = DatabaseCliServer::new(state).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

## 4. MCP Tools (14 total)

### Connection Management (no DB connection needed)

**list_configured_databases** — "List all databases from the config file. Returns name, host, port, dbname, user. Passwords are never exposed. Use this first to discover available databases."
- Input: none
- Output: `[{"name":"prod","host":"...","port":5432,"dbname":"...","user":"..."}]`

**connect_databases** — "Connect to databases by name. Names must match config entries. Connections persist across tool calls. All connections are read-only."
- Input: `{"names": ["prod", "staging"]}`
- Output: `{"connected":["prod"],"errors":[{"name":"bad","error":"..."}]}`

**disconnect_databases** — "Disconnect databases by name. Empty array disconnects all."
- Input: `{"names": ["prod"]}`
- Output: `{"disconnected":["prod"],"still_connected":["staging"]}`

**list_connected_databases** — "Show currently connected databases."
- Input: none
- Output: `[{"name":"prod","host":"...","port":5432,"dbname":"...","user":"..."}]`

### Query & Compare (connection required)

**query** — "Execute read-only SQL. Only SELECT/WITH/EXPLAIN/SHOW/TABLE allowed."
- Input: `{"sql": "SELECT ...", "database": "prod"}`
- Output: `{"database":"prod","columns":[...],"rows":[...],"row_count":N,"execution_time_ms":N}`

**compare** — "Same query across ALL connected databases. Requires 2+ connections."
- Input: `{"sql": "SELECT count(*) FROM users"}`
- Output: `{"query":"...","results":[{per db}],"errors":[{name,error}]}`

### Schema & Structure

**schema** — "Full schema: tables, columns, types, PKs, row counts, sizes."
- Input: `{"schema_name": "public", "database": "prod"}`
- Output: nested JSON with tables, columns, primary_key_columns

**sample** — "Preview rows from a table."
- Input: `{"table":"users","database":"prod","limit":20,"order_by":"created_at"}`
- Output: `{"columns":[...],"rows":[...],"total_rows_in_table":N}`

**erd** — "Entity-relationship diagram. Returns Mermaid syntax + structured data."
- Input: `{"database":"prod","schema_name":"public"}`
- Output: `{"mermaid":"erDiagram...","tables":[...],"foreign_keys":[...]}`

### Analysis

**analyze** — "Profile a table: nulls, cardinality, min/max, top values per column."
- Input: `{"table":"users","database":"prod"}`
- Output: column profiles with stats

**summary** — "Database overview: size, table count, rows, indexes, largest tables."
- Input: `{"database":"prod"}`
- Output: `{"database_name":"prod","total_size":"1.2 GB","table_count":42,...}`

**trend** — "Time-series: counts/averages grouped by day/week/month/year."
- Input: `{"table":"orders","database":"prod","timestamp_column":"created_at","interval":"day"}`
- Output: `{"rows":[{"period":"2026-03-01","count":150,"avg_value":"42.5"}]}`

**enhanced_health** — "Health of connected databases: PG version, size, uptime."
- Input: none
- Output: `[{"name":"prod","pg_version":"PostgreSQL 16.2...","db_size":"1.2 GB","uptime":"..."}]`

### Migration (analyze-only)

**suggest_migration** — "Analyze current schema and return context for migration planning. Returns schema details and FK relationships. NEVER executes DDL."
- Input: `{"database":"prod","description":"Add soft delete to users table","schema_name":"public"}`
- Output: `{"current_schema":{...},"foreign_keys":[...],"description":"...","note":"SQL below is a suggestion. NOT executed."}`

## 5. Sync/Async Bridge

Every tool that touches `ConnectionManager` uses `tokio::task::spawn_blocking`:
- Application errors (bad SQL, table not found) → `CallToolResult::success` with error text (agent sees and retries)
- Runtime errors (mutex poison, task panic) → `McpError::internal_error`

## 6. Approval Mode (Phase 3)

`--approval-mode auto|confirm`
- `auto`: all tools execute immediately (read-only enforced)
- `confirm`: SQL tools return pending approval token, agent re-calls with token

## 7. Launch

```bash
databasecli-mcp -D /project/path
```

Claude Desktop `claude_desktop_config.json`:
```json
{"mcpServers":{"databasecli":{"command":"databasecli-mcp","args":["-D","/project"]}}}
```

## 8. Phased Implementation

- **Phase 1**: Server skeleton + 7 tools (list_configured, connect, disconnect, list_connected, query, schema, sample)
- **Phase 2**: 7 more tools (analyze, compare, summary, erd, trend, health, suggest_migration)
- **Phase 3**: Approval mode with token flow
