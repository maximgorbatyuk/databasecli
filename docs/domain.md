# Domain

This repo is an operational tool for connecting to user-owned PostgreSQL databases. It does not own a business domain, persist its own data, or model business entities.

The "domain" surface that does live in the repo is small and operational:

- **Database connection profiles** — INI sections parsed from `<cwd>/.databasecli/databases.ini` into `DatabaseConfig` records (`name`, `host`, `port`, `user`, `password`, `dbname`). Owned by [`crates/databasecli-core/src/config.rs`](../crates/databasecli-core/src/config.rs).
- **Live connections** — open `postgres::Client` handles wrapped as `LiveConnection`, kept in a `ConnectionManager` keyed by profile name. Read-only by default; the local `exec` path opens a separate writable connection per call. Owned by [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs).
- **Read-only commands** — `query`, `compare`, `schema`, `sample`, `analyze`, `summary`, `erd`, `trend`, `health`, `health-check`, `list-databases`. Each is a directory entry under [`crates/databasecli-core/src/commands/`](../crates/databasecli-core/src/commands).
- **Local write/DDL command** — `exec`, in [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs). Operates on a single database, validates a deliberately narrow SQL subset, and is unreachable from MCP.
- **Init action** — bootstraps `.databasecli/databases.ini` plus per-agent MCP config files. Owned by [`crates/databasecli-core/src/init.rs`](../crates/databasecli-core/src/init.rs).

Per-type field lists, parameter signatures, and return shapes are documented next to the code (Rust doc-comments and tests in the same files). They are not duplicated here.

## Stable concepts that are useful to internalise

| Concept | Where it lives | Why it matters |
|---------|----------------|----------------|
| `StatementKind` (`Read` / `Write` / `Destructive` / `Unsupported`) | [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs) | Single source of truth for which SQL verbs `exec` accepts and which require confirmation |
| `ConnectionMode` (`ReadOnly` / `LocalExec`) | [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs) | Determines whether a connection is opened with `default_transaction_read_only = on` |
| `Settings.query_limit` | [`crates/databasecli-core/src/config.rs`](../crates/databasecli-core/src/config.rs) | Default 500, `0` = unlimited; applies to every read path including MCP |
| `CodingAgent` (`ClaudeCode` / `Cursor` / `Codex` / `Opencode`) | [`crates/databasecli-core/src/init.rs`](../crates/databasecli-core/src/init.rs) | Set of MCP-aware agents that `init` can configure; adding one means new write target |

## Cross-cutting patterns

- **No persistent state.** The tool reads INI configuration and PostgreSQL metadata; it never writes its own database, cache, or log file. The only filesystem outputs are agent config files and the initial `databases.ini` template — see [`./interactions.md`](./interactions.md).
- **No background work.** There are no schedulers, cron jobs, queues, or workers. Every action is operator-triggered.
- **Read-only by default, write-by-exception.** Default connection mode for both MCP and CLI/TUI read commands is `default_transaction_read_only = on`. Writes require the operator-only `exec` path with its own connection helper.
- **Synchronous core.** `databasecli-core` uses the synchronous `postgres` crate — no async, no `tokio` inside the core. The MCP binary uses `tokio` for the rmcp transport and bridges to sync work with `spawn_blocking`.
