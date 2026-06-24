# Architecture

For service identity, the detected stack, environment variables, and practical commands, see [`../README.md`](../README.md). For external interactions (PostgreSQL calls, MCP surface, filesystem writes), see [`./interactions.md`](./interactions.md).

## Runtime boundary

```
                               ┌──────────────────────────────┐
 operator (terminal) ───────►  │                              │
                               │   databasecli                │
                               │   (CLI dispatcher / TUI      │
                               │    via databasecli-tui)      │
                               │                              │
 AI agent (stdio JSON-RPC) ──► │   databasecli-mcp            │  ──► PostgreSQL
                               │   (rmcp + stdio)             │      (via native-tls)
                               │                              │
                               │   databasecli-core           │  ──► local filesystem:
                               │   (config, connection,       │      .databasecli/databases.ini
                               │    health, query, exec,      │      .mcp.json, .cursor/mcp.json
                               │    export, schema, ...)      │      opencode.jsonc, .codex/config.toml
                               └──────────────────────────────┘
```

Both binaries link `databasecli-core` directly. There is no network listener exposed by either binary — `databasecli-mcp` speaks JSON-RPC over stdio.

## Workspace layout

The repo is a Cargo workspace defined in [`../Cargo.toml`](../Cargo.toml). All crates inherit `version`, `edition`, `license`, `repository`, and `authors` from the workspace package.

| Crate | Type | Purpose |
|-------|------|---------|
| [`crates/databasecli-cli`](../crates/databasecli-cli) | Binary (`databasecli`) | `clap` parsing, dispatch to TUI or to subcommand handlers in `databasecli-core` |
| [`crates/databasecli-core`](../crates/databasecli-core) | Library | All domain logic: config parsing, connection pooling/lifecycle, health checks, schema dump, query/exec validators, analysis commands, init writers |
| [`crates/databasecli-tui`](../crates/databasecli-tui) | Library (consumed by the CLI binary) | TUI app state, event loop, `ratatui` rendering, background-thread orchestration |
| [`crates/databasecli-mcp`](../crates/databasecli-mcp) | Binary (`databasecli-mcp`) | `rmcp` server bindings, stdio transport, per-tool wrappers around `databasecli-core` services |

`default-members = ["crates/databasecli-cli"]` — `cargo run` and `cargo build` without a `-p` flag target the CLI binary only.

## Internal dependencies

```
databasecli-cli ──► databasecli-core
databasecli-cli ──► databasecli-tui ──► databasecli-core
databasecli-mcp ──► databasecli-core
```

All four crates depend on `databasecli-core`; nothing depends on `databasecli-cli` or `databasecli-mcp`. `databasecli-tui` is a thin presentation layer — it never opens its own PostgreSQL connection except to delegate to `connect_for_local_exec` for the `Execute` screen, and it never re-implements any validator. The MCP crate has no UI code and no awareness of the TUI.

## Layer responsibilities

- **databasecli-core** owns every fact that touches PostgreSQL or the INI config: connection setup, transaction modes, validators, command implementations, formatters used by both the CLI and TUI. Errors crossing crate boundaries are typed via `thiserror` (`DatabaseCliError`).
- **databasecli-cli** is a `clap`-driven dispatcher. Every subcommand handler lives in [`crates/databasecli-cli/src/run.rs`](../crates/databasecli-cli/src/run.rs) and is a thin wrapper that loads config, opens connections via `ConnectionManager` (or `connect_for_local_exec` for `exec`), calls a service in `databasecli-core`, and prints the formatted result.
- **databasecli-tui** uses `ratatui` for rendering and `crossterm` for input. The TUI runs synchronously in the foreground, dispatches blocking work to background threads via `mpsc::channel`, and polls input with a 100 ms timeout so the spinner stays animated. The state machine is in [`crates/databasecli-tui/src/app.rs`](../crates/databasecli-tui/src/app.rs); rendering per-screen is split into one module per screen under [`crates/databasecli-tui/src/ui/`](../crates/databasecli-tui/src/ui).
- **databasecli-mcp** wraps each MCP tool in an async function in [`crates/databasecli-mcp/src/tools/`](../crates/databasecli-mcp/src/tools), with the public schema and tool registration generated from `#[tool]`-annotated methods on `DatabaseCliServer` in [`crates/databasecli-mcp/src/server.rs`](../crates/databasecli-mcp/src/server.rs). Synchronous PostgreSQL work runs through `tokio::task::spawn_blocking` — see `McpSessionState::with_manager` in [`crates/databasecli-mcp/src/state.rs`](../crates/databasecli-mcp/src/state.rs).

## Stable architectural surfaces

| Surface | Location | Why it is stable |
|---------|----------|------------------|
| CLI argument structure | [`crates/databasecli-cli/src/args.rs`](../crates/databasecli-cli/src/args.rs) | Public CLI contract used by users, scripts, and the help screen |
| TUI dispatcher | [`crates/databasecli-tui/src/lib.rs`](../crates/databasecli-tui/src/lib.rs) | Owns the foreground/background-thread split and ties screens to background results |
| TUI screen registry | [`crates/databasecli-tui/src/ui/mod.rs`](../crates/databasecli-tui/src/ui) and [`crates/databasecli-tui/src/event/mod.rs`](../crates/databasecli-tui/src/event) | Adding a screen requires editing both modules |
| MCP server registration | [`crates/databasecli-mcp/src/server.rs`](../crates/databasecli-mcp/src/server.rs) | Every `#[tool]` `fn` is allowlisted by [`crates/databasecli-mcp/tests/guard.rs`](../crates/databasecli-mcp/tests/guard.rs) — adding a tool requires both files |
| MCP read-only guard | [`crates/databasecli-mcp/tests/guard.rs`](../crates/databasecli-mcp/tests/guard.rs) | Build-time enforcement that the MCP crate never references `execute_statement` |
| Connection mode helper | [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs) | Single place that decides whether a connection is read-only or write-capable |
| Read-only validator | [`crates/databasecli-core/src/commands/query.rs`](../crates/databasecli-core/src/commands/query.rs) (`validate_readonly`) | Guards every read path including MCP `query` and `compare` |
| Local-exec validator | [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs) (`validate_single_statement`, `classify_keyword`) | Guards every write path |
| Init config writers | [`crates/databasecli-core/src/init.rs`](../crates/databasecli-core/src/init.rs) | Owns the format of every agent config file the tool emits |
| CI definition | [`../.github/workflows/ci.yml`](../.github/workflows/ci.yml) | Defines what "green" means on Linux and Windows |
| Release pipeline | [`../.github/workflows/release.yml`](../.github/workflows/release.yml), [`../dist-workspace.toml`](../dist-workspace.toml), [`../scripts/release.py`](../scripts/release.py) | Defines distribution targets, installers, and tap |

## Execution flow

### CLI subcommand

```
operator → cargo / installed binary
  → crates/databasecli-cli/src/main.rs (clap parse)
  → crates/databasecli-cli/src/run.rs (handler)
  → databasecli-core::config (resolve & load INI)
  → databasecli-core::connection (ConnectionManager or connect_for_local_exec)
  → databasecli-core::commands::<verb> (validation, SQL, formatting)
  → stdout
```

`commands::render` is the shared formatting helper used by the table renderers in `query`/`sample`/`exec` (cell clipping to avoid the `u16` format-width panic) and by the csv/tsv field quoting in `query` and `export`.

### Read-only `export` (streaming cursor)

```
operator → databasecli export --db <name> <table>|--query "<SQL>" [--format csv|jsonl|sql] [--output FILE]
  → run_export (crates/databasecli-cli/src/run.rs)
  → commands::export::{table_request | query_request} (validate_identifier / validate_readonly)
  → ConnectionManager (read-only connection; exactly one --db required)
  → commands::export::export — DECLARE NO SCROLL CURSOR + batched FETCH FORWARD inside a transaction
  → stream rows to FILE or stdout (never materialized; ignores query_limit)
```

This path is CLI/TUI-only and never wired into the MCP surface.

### TUI

```
operator → databasecli (no args) or `databasecli tui`
  → crates/databasecli-cli/src/main.rs → databasecli_tui::run(directory)
  → enable_raw_mode + enter alternate screen
  → run_loop:
      crossterm::poll(100ms)
        → event::handle_key (per-screen)
        → app.take_action() yields a pending AppAction
        → AppAction dispatches to a background thread
        → background result returns via mpsc::channel
      ratatui::draw → ui::draw (per-screen)
  → quit → leave alternate screen, disable raw mode
```

### MCP request

```
AI agent (stdio) → rmcp transport
  → DatabaseCliServer (server.rs)
  → tools::<area>::<fn> (per-tool wrapper)
  → McpSessionState::with_manager — spawns blocking task
  → databasecli-core::commands::<verb>
  → JSON response on stdout
```

### Local `exec` (inline)

```
operator → databasecli exec --db <name> [--yes] "<SQL>"
  → run_exec → run_exec_inline (crates/databasecli-cli/src/run.rs)
  → validate_single_statement (single stmt; resolves WITH ... DML via resolve_with_kind;
                               rejects dollar-quoted bodies and multi-statement)
  → NormalizedStatement.kind → Read | Write | Destructive | Unsupported
  → if Destructive: prompt unless --yes (or fail with ExecConfirmationRequired in non-interactive)
  → connect_for_local_exec (fresh writable connection; statement_timeout from [settings]/--timeout, default 30s)
  → execute_normalized → either Client::execute or Client::query (when top-level RETURNING is present)
  → format_execute_result → stdout
```

### Local `exec --file`

```
operator → databasecli exec --db <name> --file <PATH> [--transaction] [--yes]
  → run_exec → run_exec_file
  → fs::read_to_string
  → split_script (string-literal/comment aware; rejects dollar-quoted bodies)
  → reject Read/Unsupported chunks with line context
  → optionally wrap in injected BEGIN/COMMIT (when --transaction)
  → scan for Destructive chunks → single prompt listing all with line numbers (unless --yes)
  → connect_for_local_exec (one fresh writable connection for the whole script)
  → execute_script (sequential execute_normalized per chunk; stops at first error)
  → format_script_results → stdout (per-statement header + body)
```

The TUI `Execute` screen drives the same flow via `AppAction::ExecuteScript`. The multi-line buffer is split with `split_script` exactly as `--file` does; bracketed paste keeps newlines intact when a script is pasted from the clipboard.

## Pipeline / ordering details

- The TUI event loop polls input with a 100 ms timeout so the spinner animates while a background thread is in flight. Every key event goes through `handle_key` in [`crates/databasecli-tui/src/event/mod.rs`](../crates/databasecli-tui/src/event/mod.rs), which (a) filters non-`Press` events, and (b) normalises Cyrillic ЙЦУКЕН keys to their QWERTY equivalents *unless* a text-input buffer is active. Any new TUI key handler must respect both rules.
- The MCP server registers tools through `#[tool_router]` and `#[tool]` macros from `rmcp`. The order of `#[tool]` methods on `DatabaseCliServer` does not matter to clients, but every `fn` in `server.rs` must appear in the `ALLOWED` list inside `tests/guard.rs`. The guard test fails the build otherwise.
- `validate_single_statement` runs against a comment-stripped *analysis copy* of the SQL, but builds the executable string from the *original* operator input (only trimming whitespace and removing at most one trailing semicolon). This invariant is documented in code comments in [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs); breaking it can silently widen a destructive statement.

## Deployment / distribution

This repo ships binaries; it does not deploy a service. Distribution is owned by `cargo-dist` and configured in [`../dist-workspace.toml`](../dist-workspace.toml).

| Resource | Configuration | Purpose |
|----------|---------------|---------|
| Release tag pattern | `**[0-9]+.[0-9]+.[0-9]+*` (any tag matching SemVer) | Triggers [`../.github/workflows/release.yml`](../.github/workflows/release.yml) |
| Installers built | `shell`, `powershell`, `homebrew`, `msi` | [`../dist-workspace.toml`](../dist-workspace.toml) |
| Build targets | `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` | [`../dist-workspace.toml`](../dist-workspace.toml) |
| Homebrew tap | `maximgorbatyuk/homebrew-tap` | [`../dist-workspace.toml`](../dist-workspace.toml) |
| Release driver | [`../scripts/release.py`](../scripts/release.py) | Validates `CHANGELOG.md`, bumps version in workspace `Cargo.toml`, runs `just verify`, pushes `dev` and `main`, creates and pushes the version tag |
| WiX (MSI) inputs | [`../crates/databasecli-cli/wix/main.wxs`](../crates/databasecli-cli/wix/main.wxs), [`../crates/databasecli-mcp/wix/main.wxs`](../crates/databasecli-mcp/wix/main.wxs) | Windows MSI installer definitions |

## Dependency categories

Versions are pinned in [`../Cargo.lock`](../Cargo.lock) — refer there for current numbers.

### First-party / shared (none)

No internal shared crates outside this workspace and no private registry. All cross-crate sharing is through the in-workspace `databasecli-core` library.

### Third-party runtime

| Dependency | Crate(s) using it | Why |
|------------|-------------------|-----|
| `clap` (workspace dep) | `databasecli-cli`, `databasecli-mcp` | CLI argument parsing |
| `anyhow` (workspace dep) | `databasecli-cli`, `databasecli-mcp`, `databasecli-tui` | Error wrapping at the binary boundary |
| `thiserror` | `databasecli-core` | Typed `DatabaseCliError` for cross-crate boundaries |
| `postgres`, `postgres-native-tls`, `native-tls` | `databasecli-core` | Synchronous PostgreSQL client + TLS |
| `configparser` | `databasecli-core` | INI parsing |
| `serde_json`, `toml` | `databasecli-core`, `databasecli-mcp` | Config emission for MCP-aware agents and JSON responses |
| `chrono`, `uuid` | `databasecli-core` | PostgreSQL type mappings (`TIMESTAMPTZ`, `UUID`, etc.) |
| `home` | `databasecli-core` | `~` expansion in `-D` paths |
| `crossterm`, `ratatui` | `databasecli-tui` | Terminal input + rendering |
| `rmcp`, `schemars`, `serde` | `databasecli-mcp` | MCP server, JSON schema for tool params |
| `tokio` (`full` features) | `databasecli-mcp` | Async runtime; required by `rmcp`. Sync work runs under `tokio::task::spawn_blocking` |
| `tracing`, `tracing-subscriber` | `databasecli-mcp` | stderr-only logging for the MCP server |

### Build / test / dev-only

| Dependency | Used by | Why |
|------------|---------|-----|
| `tempfile` | `databasecli-core` (dev-deps) | Test fixtures for INI parsing and init writers |
| `cargo-dist` | release pipeline | Cross-platform installer generation |
| `just` | dev workflow | Aggregated `verify`, `lint`, `test`, `build`, `dist-plan` targets |
