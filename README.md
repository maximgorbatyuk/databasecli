# databasecli

Manage your databases with AI agents. A CLI, TUI, and MCP server providing secure read-only access to PostgreSQL databases.

## Service identity

| Field | Value |
|-------|-------|
| Name | databasecli |
| Repository | [`maximgorbatyuk/databasecli`](https://github.com/maximgorbatyuk/databasecli) |
| Primary language | Rust (edition 2024) — see [`Cargo.toml`](Cargo.toml) |
| Workspace shape | Cargo workspace with one binary crate (`databasecli`), one binary MCP crate (`databasecli-mcp`), and two library crates (`databasecli-core`, `databasecli-tui`) — see [`Cargo.toml`](Cargo.toml) and [`crates/`](crates) |
| Persistence | PostgreSQL (synchronous `postgres` crate, TLS via `native-tls`) |
| Integration style | stdio JSON-RPC (MCP), local CLI/TUI |
| Primary entrypoints | [`crates/databasecli-cli/src/main.rs`](crates/databasecli-cli/src/main.rs) (CLI/TUI), [`crates/databasecli-mcp/src/main.rs`](crates/databasecli-mcp/src/main.rs) (MCP server) |

## Detected stack

| Concern | Tool | Source of truth |
|---------|------|-----------------|
| Package / build manager | Cargo workspace | [`Cargo.toml`](Cargo.toml) |
| Task runner | `just` | [`justfile`](justfile) |
| Test runner | `cargo test` | [`.github/workflows/ci.yml`](.github/workflows/ci.yml) |
| Linter | `cargo clippy` (warnings as errors) | [`justfile`](justfile), [`.github/workflows/ci.yml`](.github/workflows/ci.yml) |
| Formatter | `rustfmt` (`cargo fmt`) | [`justfile`](justfile) |
| TUI framework | `crossterm` + `ratatui` | [`crates/databasecli-tui/Cargo.toml`](crates/databasecli-tui/Cargo.toml) |
| MCP framework | `rmcp` (server, stdio transport) | [`crates/databasecli-mcp/Cargo.toml`](crates/databasecli-mcp/Cargo.toml) |
| Async runtime | `tokio` (MCP server only — CLI/TUI/core are synchronous) | [`crates/databasecli-mcp/Cargo.toml`](crates/databasecli-mcp/Cargo.toml) |
| Distribution | `cargo-dist` + GitHub Actions | [`dist-workspace.toml`](dist-workspace.toml), [`.github/workflows/release.yml`](.github/workflows/release.yml) |
| Homebrew tap | `maximgorbatyuk/homebrew-tap` | [`dist-workspace.toml`](dist-workspace.toml) |
| Editor config | `.editorconfig` (LF, UTF-8, trim trailing whitespace) | [`.editorconfig`](.editorconfig) |

## What this tool does

1. Stores PostgreSQL connection profiles in a project-local INI file (`<cwd>/.databasecli/databases.ini`).
2. Ships a TUI (default mode) and CLI subcommands for connection management, health checks, schema inspection, and read-only querying across one or many databases.
3. Provides a separate `databasecli-mcp` binary that exposes the same read-only capabilities as MCP tools over stdio so AI agents (Claude Code, Claude Desktop, Cursor, Codex, Opencode) can explore PostgreSQL safely.
4. Supports a deliberately narrow local `exec` path for one-shot write/DDL statements with destructive-statement confirmation. This path is unreachable from MCP by construction.
5. Bootstraps configuration for the agents above through `databasecli init`.

## Start here

- Contribution rules and guard rails: [`AGENTS.md`](AGENTS.md).
- Workspace structure, dependency categories, runtime topology: [`docs/architecture.md`](docs/architecture.md).

## Documentation index

| Document | Description |
|----------|-------------|
| [`AGENTS.md`](AGENTS.md) | Contribution rules and required updates per change type |
| [`docs/architecture.md`](docs/architecture.md) | Crates, runtime topology, execution flow, deployment, dependency categories |
| [`docs/domain.md`](docs/domain.md) | Domain surfaces (light — this is operational tooling, not a domain model) |
| [`docs/interactions.md`](docs/interactions.md) | PostgreSQL calls, MCP stdio surface, filesystem writes, no-event-bus declaration |
| [`docs/testing.md`](docs/testing.md) | Test layout, runners, fixtures, MCP guard tests |
| [`docs/gotchas.md`](docs/gotchas.md) | Non-obvious constraints (TLS, Windows key events, validator quirks, write-path isolation) |
| [`docs/mcp.md`](docs/mcp.md) | Detailed MCP server reference: tool catalogue, security model, agent setup |
| [`docs/plans/execution.md`](docs/plans/execution.md) | Original implementation plan for the local SQL `exec` feature (historical) |
| `docs/authentication.md` | Omitted — repo has no application auth/authorization logic; PostgreSQL credentials are passed to the driver as-is |

## Installation

### macOS (Homebrew)

```bash
brew tap maximgorbatyuk/tap
brew install databasecli
brew install databasecli-mcp   # MCP server for AI agents
databasecli --version
```

### Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/maximgorbatyuk/databasecli/releases/latest/download/databasecli-installer.sh | sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/maximgorbatyuk/databasecli/releases/latest/download/databasecli-mcp-installer.sh | sh
```

### Windows (PowerShell)

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/maximgorbatyuk/databasecli/releases/latest/download/databasecli-installer.ps1 | iex"
powershell -ExecutionPolicy ByPass -c "irm https://github.com/maximgorbatyuk/databasecli/releases/latest/download/databasecli-mcp-installer.ps1 | iex"
```

MSI installers are also published on the [releases page](https://github.com/maximgorbatyuk/databasecli/releases).

### From source

```bash
cargo install --git https://github.com/maximgorbatyuk/databasecli databasecli
cargo install --git https://github.com/maximgorbatyuk/databasecli databasecli-mcp
```

## Dependency setup

The repo builds with a stock Rust toolchain — install via `rustup`. There is no separate package manager, no Node toolchain, no Python build dependency for the binaries themselves (the optional release script is Python 3 + `gh`). Required tooling:

| Tool | Required for | How to get it |
|------|--------------|---------------|
| Rust toolchain (stable, edition 2024) | build, test, lint | `rustup` |
| `cargo` | all build/test/lint commands | bundled with Rust toolchain |
| `just` (optional) | running aggregated `just verify` etc. | `cargo install just` or Homebrew |
| `cargo-dist` (release-time only) | reproducing the release pipeline | `cargo install cargo-dist@0.31.0` (matches [`dist-workspace.toml`](dist-workspace.toml)) |
| `gh` + Python 3.10+ (release-time only) | `scripts/release.py` | `brew install gh`, system Python |
| Reachable PostgreSQL server | live `health`, `query`, `exec`, MCP usage | bring your own; no test container is provisioned in this repo |

There are no first-party shared crates outside this workspace and no private registries. All dependencies are pulled from `crates.io`; current versions are pinned in [`Cargo.lock`](Cargo.lock).

## Commands

All commands run from the repository root unless noted otherwise.

| Purpose | Command | Source of truth |
|---------|---------|-----------------|
| Run full verification (fmt, clippy, test, check) | `just verify` | [`justfile`](justfile) |
| Format check | `cargo fmt --all --check` | [`justfile`](justfile), [`.github/workflows/ci.yml`](.github/workflows/ci.yml) |
| Lint (warnings = errors) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | [`justfile`](justfile), [`.github/workflows/ci.yml`](.github/workflows/ci.yml) |
| Run all tests | `cargo test --workspace` | [`justfile`](justfile), [`.github/workflows/ci.yml`](.github/workflows/ci.yml) |
| Build (debug) | `cargo build --workspace` | [`Cargo.toml`](Cargo.toml) |
| Build (release) | `cargo build --workspace --release` | [`justfile`](justfile) |
| Run the CLI/TUI from source | `cargo run` (alias for `cargo run -p databasecli`) | [`Cargo.toml`](Cargo.toml) `default-members` |
| Run the MCP server from source | `cargo run -p databasecli-mcp` | [`crates/databasecli-mcp/Cargo.toml`](crates/databasecli-mcp/Cargo.toml) |
| Bootstrap config + agent MCP entries | `databasecli init` | [`crates/databasecli-core/src/init.rs`](crates/databasecli-core/src/init.rs) |
| List configured databases | `databasecli list` | [`crates/databasecli-cli/src/args.rs`](crates/databasecli-cli/src/args.rs) |
| Health check (basic) | `databasecli health` | [`crates/databasecli-core/src/health.rs`](crates/databasecli-core/src/health.rs) |
| Enhanced health (version, size, uptime) | `databasecli health-check --all` | [`crates/databasecli-core/src/commands/health.rs`](crates/databasecli-core/src/commands/health.rs) |
| Read-only SQL | `databasecli query --db <name> "<SQL>"` | [`crates/databasecli-core/src/commands/query.rs`](crates/databasecli-core/src/commands/query.rs) |
| Local-only write/DDL (inline) | `databasecli exec --db <name> [--yes] "<SQL>"` | [`crates/databasecli-core/src/commands/execute.rs`](crates/databasecli-core/src/commands/execute.rs) |
| Local-only write/DDL (script) | `databasecli exec --db <name> --file <PATH> [--transaction] [--yes]` | [`crates/databasecli-core/src/commands/execute.rs`](crates/databasecli-core/src/commands/execute.rs) |
| Full reference of CLI subcommands | `databasecli reference` | [`crates/databasecli-core/src/help.rs`](crates/databasecli-core/src/help.rs) |
| Cut a release | `./scripts/release.py [X.Y.Z]` | [`scripts/release.py`](scripts/release.py) |

`cargo run` requires a working `.databasecli/databases.ini` to do anything useful — see Quick Start.

## Quick start

### Option 1 — `databasecli init`

```bash
databasecli init
```

The command interactively asks which coding agents to configure for MCP, creates `<cwd>/.databasecli/databases.ini` with a template, and writes the agent-specific MCP config files (`.mcp.json`, `opencode.jsonc`, `.codex/config.toml`, `.cursor/mcp.json` — see [`docs/interactions.md`](docs/interactions.md)).

### Option 2 — manual setup

1. Create `.databasecli/databases.ini` in your project directory:

   ```ini
   [production]
   host = localhost
   port = 5432
   user = admin
   password = secret123
   dbname = myapp

   [staging]
   host = staging-db.example.com
   port = 5432
   user = readonly
   password = secret456
   dbname = myapp_staging
   ```

2. Create `.mcp.json` in your project root if you want AI-agent access:

   ```json
   {
     "mcpServers": {
       "databasecli": {
         "command": "databasecli-mcp",
         "args": ["-D", "."]
       }
     }
   }
   ```

3. Run:

   ```bash
   databasecli                              # launch TUI
   databasecli list                         # list stored connections
   databasecli health-check --all           # check all databases
   databasecli schema --db production       # inspect schema
   databasecli query --db production "SELECT count(*) FROM users"

   # Local-only write/DDL execution. Asks `[y/N]` before destructive statements;
   # pass --yes to bypass. Not exposed via MCP.
   databasecli exec --db staging "INSERT INTO feature_flags (name) VALUES ('beta')"
   databasecli exec --db staging --yes "DELETE FROM sessions WHERE expired_at < now()"

   # WITH ... DML chains are accepted (severity is the worst verb across all CTEs).
   databasecli exec --db staging --yes \
     "WITH ev AS (INSERT INTO events (slug) VALUES ('dev') RETURNING id)
      INSERT INTO log (event_id) SELECT id FROM ev"

   # Multi-statement scripts: BEGIN/COMMIT, SAVEPOINT, SET, seed fixtures.
   databasecli exec --db staging --file db/seed.sql
   databasecli exec --db staging --file db/migrate.sql --transaction --yes
   ```

### `query` vs `exec`

| Need | Use | Notes |
|------|-----|-------|
| Read data (`SELECT`, `SHOW`, `EXPLAIN`, `TABLE`, `WITH ... SELECT`) | `databasecli query` | Read-only path; supports `--db <name>` and `--all` for multi-database workflows |
| Change schema/data (`INSERT`, `UPDATE`, `DELETE`, `ALTER`, `CREATE`, `WITH ... DML`, ...) | `databasecli exec` | Local-only path on one database; destructive verbs prompt unless `--yes` |

The inline form accepts one statement (including a `WITH ... INSERT|UPDATE|DELETE` chain). The `--file` form accepts a multi-statement script with transaction control (`BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT`/`SET`). Procedural bodies (`DO $$ ... $$`, function definitions) and `COPY` remain unsupported in both forms — see [`docs/gotchas.md`](docs/gotchas.md) for the full list.

## MCP server

The `databasecli-mcp` binary is a read-only MCP server that gives AI agents secure access to your PostgreSQL databases over stdio. All connections enforce read-only mode at both the validator and database level. For the full tool catalogue and security model, see [`docs/mcp.md`](docs/mcp.md). The behavioural guarantee that MCP cannot run write statements is enforced by guard tests in [`crates/databasecli-mcp/tests/guard.rs`](crates/databasecli-mcp/tests/guard.rs).

## Local app lifecycle

| Process | What it serves | Foreground? | Stop with |
|---------|----------------|-------------|-----------|
| `databasecli` | Full-screen TUI on the current terminal | yes — alternate screen | `q` or `Ctrl-C` |
| `databasecli list`, `health`, `health-check`, `schema`, `query`, `exec`, `summary`, `erd`, `analyze`, `compare`, `trend`, `sample` | One-shot output to stdout | no | exits when done |
| `databasecli-mcp` | stdio JSON-RPC (no socket, no port) | yes — controlled by the MCP client | the MCP client closes stdin |

There are no listening ports, no background workers, and no shared local services owned by this repo. PostgreSQL is the only network dependency and it is brought by the user. Connection timeout is 5 seconds and statement timeout is 30 seconds — see [`docs/gotchas.md`](docs/gotchas.md).

## Environment variables

| Variable | Required? | Purpose | Source |
|----------|-----------|---------|--------|
| `DATABASECLI_CONFIG_PATH` | No | Overrides the resolved INI path; takes priority over `-D` and the default `<cwd>/.databasecli/databases.ini`. Mainly used by tests. | [`crates/databasecli-core/src/config.rs`](crates/databasecli-core/src/config.rs) |
| `RUST_LOG` | No | Standard `tracing-subscriber` env filter for the MCP server only. CLI/TUI do not use `tracing`. | [`crates/databasecli-mcp/src/main.rs`](crates/databasecli-mcp/src/main.rs) |

## Configuration file naming

The repo emits configuration files in user projects (via `databasecli init`). Resource names follow these patterns:

| Resource | Path pattern | Purpose |
|----------|--------------|---------|
| INI config | `<cwd>/.databasecli/databases.ini` (or `<-D>/.databasecli/databases.ini`, or `$DATABASECLI_CONFIG_PATH`) | Per-database connection profiles plus optional `[settings]` section |
| Claude Code MCP | `<project>/.mcp.json` | `mcpServers.databasecli` entry |
| Cursor MCP | `<project>/.cursor/mcp.json` | `mcpServers.databasecli` entry |
| Codex MCP | `<project>/.codex/config.toml` | `[mcp_servers.databasecli]` entry |
| Opencode MCP | `<project>/opencode.jsonc` | `mcp.databasecli` entry |

The exact contents written by each path live in [`crates/databasecli-core/src/init.rs`](crates/databasecli-core/src/init.rs); see also [`docs/interactions.md`](docs/interactions.md).

## License

MIT. See [LICENSE](LICENSE).
