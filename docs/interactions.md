# External interactions

For the runtime boundary diagram and execution flows, see [`./architecture.md`](./architecture.md).

This repo has three kinds of external interactions: synchronous PostgreSQL calls, the MCP stdio surface (incoming JSON-RPC from AI agents), and filesystem writes for config bootstrapping. There are no published or consumed events, no message bus, no webhooks, no schedulers, and no listening network sockets.

## Runtime calls — PostgreSQL

| Path | Caller | Purpose |
|------|--------|---------|
| `postgres::Client::connect` (TLS via `postgres-native-tls`) | [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs), [`crates/databasecli-core/src/health.rs`](../crates/databasecli-core/src/health.rs) | Open a synchronous connection. Connect timeout is 5 seconds (encoded in the connection string built by `DatabaseConfig::connection_string`). |
| `SET default_transaction_read_only = on; SET statement_timeout = '30s'` | `open_client(.., ConnectionMode::ReadOnly)` | Applied to every connection used by `ConnectionManager` (CLI/TUI read commands and all MCP tools). |
| `SET statement_timeout = '30s'` | `connect_for_local_exec` | Applied to every short-lived writable connection used by the local `exec` path. Inline `exec` uses one connection per statement; `exec --file` uses one connection per script invocation so BEGIN/COMMIT/SAVEPOINT work as written. |
| `Client::query`, `Client::execute`, `Client::prepare`, `Client::simple_query` | command modules under [`crates/databasecli-core/src/commands/`](../crates/databasecli-core/src/commands) and [`crates/databasecli-core/src/health.rs`](../crates/databasecli-core/src/health.rs) | Read SQL, write SQL (RETURNING vs non-RETURNING), and the per-database `SELECT 1` heartbeat used by `check_health`. |

TLS uses `native_tls::TlsConnector::builder().danger_accept_invalid_certs(true)`. Connections are encrypted but not CA-verified — see [`./gotchas.md`](./gotchas.md).

PostgreSQL is the only external runtime dependency. There are no HTTP clients, no SDKs, no third-party APIs, no caches, no search backends, and no payment/email/analytics integrations.

## Incoming integrations — MCP stdio surface

`databasecli-mcp` runs as a child process of the user's MCP client (Claude Desktop, Claude Code, Cursor, Codex, Opencode, ...). The transport is stdio JSON-RPC; no port is opened. The server is registered by [`crates/databasecli-mcp/src/server.rs`](../crates/databasecli-mcp/src/server.rs) using `rmcp` macros. Each tool is implemented in [`crates/databasecli-mcp/src/tools/`](../crates/databasecli-mcp/src/tools).

| Tool | Owning module | Purpose |
|------|---------------|---------|
| `list_configured_databases` | [`tools/connection.rs`](../crates/databasecli-mcp/src/tools/connection.rs) | List INI sections (passwords excluded) |
| `connect_databases` | [`tools/connection.rs`](../crates/databasecli-mcp/src/tools/connection.rs) | Open read-only connections to one or more configured databases |
| `disconnect_databases` | [`tools/connection.rs`](../crates/databasecli-mcp/src/tools/connection.rs) | Close named connections, or all connections when given an empty list |
| `list_connected_databases` | [`tools/connection.rs`](../crates/databasecli-mcp/src/tools/connection.rs) | List currently active connections |
| `query` | [`tools/query.rs`](../crates/databasecli-mcp/src/tools/query.rs) | Run a read-only statement against one connected database, with `validate_readonly` enforcement |
| `compare` | [`tools/query.rs`](../crates/databasecli-mcp/src/tools/query.rs) | Run the same read-only statement across all connected databases |
| `schema`, `sample`, `erd` | [`tools/schema.rs`](../crates/databasecli-mcp/src/tools/schema.rs) | Schema dump, table preview, ERD generation |
| `analyze`, `summary`, `trend`, `enhanced_health` | [`tools/analysis.rs`](../crates/databasecli-mcp/src/tools/analysis.rs) | Per-table profile, database overview, time-series counts/averages, health snapshot |
| `suggest_migration` | [`tools/migration.rs`](../crates/databasecli-mcp/src/tools/migration.rs) | Returns schema context for migration planning. **Never executes DDL** — see [`./gotchas.md`](./gotchas.md) |

Adding a new tool means: (1) declare it on `DatabaseCliServer` with `#[tool(...)]`, (2) implement the wrapper in `tools/<area>.rs`, (3) add the `fn` name to the `ALLOWED` list in [`crates/databasecli-mcp/tests/guard.rs`](../crates/databasecli-mcp/tests/guard.rs). The guard test will fail the build if step 3 is skipped.

The MCP surface is strictly read-only. Writes are not exposed; the validator in [`crates/databasecli-core/src/commands/query.rs`](../crates/databasecli-core/src/commands/query.rs) (`validate_readonly`) plus the database-level read-only transaction setting plus the guard tests in [`crates/databasecli-mcp/tests/guard.rs`](../crates/databasecli-mcp/tests/guard.rs) form three independent layers. Full security model: [`./mcp.md`](./mcp.md).

## Filesystem reads — `exec --file`

`databasecli exec --db <name> --file <PATH>` reads `PATH` via `std::fs::read_to_string` once at the start of the run. The file is parsed by `split_script` into single statements (string-literal and comment aware; dollar-quoted bodies rejected up front), and every statement runs on the same short-lived writable connection. The read is a one-shot synchronous operation — there is no streaming, no inotify-style watch, and no temp file fan-out. The TUI Execute screen performs no filesystem read of its own; instead, terminal bracketed paste delivers script content via `Event::Paste(String)` and the buffer is split the same way.

## Filesystem writes — `init`

Running `databasecli init` writes config files into the operator's project. The exact contents and idempotency rules live in [`crates/databasecli-core/src/init.rs`](../crates/databasecli-core/src/init.rs); the schema for each is verified by tests in the same file.

| Target path | Format | Owning helper |
|-------------|--------|---------------|
| `<base>/.databasecli/databases.ini` | INI template (commented) | `create_default_config` |
| `<base>/.mcp.json` (Claude Code) | JSON: `mcpServers.databasecli` entry | `upsert_claude_code` |
| `<base>/.cursor/mcp.json` (Cursor) | JSON: `mcpServers.databasecli` entry (same shape as Claude Code) | `upsert_claude_code` (reused) |
| `<base>/.codex/config.toml` (Codex) | TOML: `[mcp_servers.databasecli]` table | `upsert_codex` |
| `<base>/opencode.jsonc` (Opencode) | JSONC: `mcp.databasecli` entry | `upsert_opencode` |

`<base>` defaults to the current working directory and is overridden by the `-D` flag (with `~` expansion). All upserts are idempotent — re-running `init` reports `Unchanged` when the entry is already present.

The CLI dispatcher in [`crates/databasecli-cli/src/run.rs`](../crates/databasecli-cli/src/run.rs) (`run_init`) and the TUI `Init` action in [`crates/databasecli-tui/src/lib.rs`](../crates/databasecli-tui/src/lib.rs) both delegate to `databasecli_core::init::run_init`.

## Outgoing integrations — none

There are no outbound HTTP requests at runtime, no published events, no webhooks fired, no cloud SDK calls, no analytics or telemetry, and no email or notification backends. The release pipeline (see [`./architecture.md`](./architecture.md)) makes outbound calls during CI/CD, but those are part of distribution, not runtime.

## Scheduled / background work

None. Every operation is operator-triggered. The TUI runs background threads only for the duration of the active screen's work and joins via `mpsc::channel`.

## Health checks / readiness

`check_health` (basic) and `check_all_enhanced_health` (extended: PostgreSQL version, database size, server uptime) are operator-triggered probes — there is no exposed `/health` endpoint and no automated heartbeat. The basic `check_health` opens a fresh connection, runs `SELECT 1`, and reports response time and error string.
