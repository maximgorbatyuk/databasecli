# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.9] - 2026-06-24

### Fixed

- **CLI no longer panics on wide result cells**: rendering a very wide value (e.g. a ~300 KB `jsonb` cell) previously crashed with `Formatting argument out of range` because the dynamic `format!` width exceeded `u16::MAX`. A shared `commands::render::table_cell` now clips every cell to `MAX_COL_WIDTH` (200 chars) with an ellipsis and collapses embedded newlines/tabs before measuring or padding. The fix covers `query`, `sample`, and `exec` table output. Regression test renders `SELECT repeat('A',300000)` without panic.
- **Malformed multi-statement error message**: submitting SQL with an internal `;` returned a garbled message that spliced the rule text into the read-only template. It now returns a dedicated `DatabaseCliError::MultiStatement` with a clear message. A single trailing `;` (`SELECT … ;`) is now tolerated on the read-only path (CLI and MCP).

### Added

- **`databasecli query` output formats**: `--format table|csv|tsv|json|ndjson` plus `--no-header`. Row data goes to stdout; row count, timing, and truncation notices go to stderr so piped output stays clean. CSV/TSV use RFC 4180 quoting, fixing embedded newlines/tabs/commas that previously broke line/column parsing. SQL `NULL` is now distinct from data across all formats: the literal text `NULL` in tables, an empty field in csv/tsv, and json `null` in json/ndjson (matching `export`); this also corrects the MCP `query` JSON, which previously emitted NULL as the string `"NULL"`. With `--all-databases`, json prints one document per database — use ndjson for a single combined stream.
- **`databasecli query --limit N`**: override `[settings] query_limit` per run (`0` = unlimited). The truncation notice now points at `--limit`/`LIMIT`/`OFFSET`/`query_limit`.
- **`databasecli export <table>|--query <SQL> --format csv|jsonl|sql [--output FILE] [--schema <name>]`**: stream a table or read-only query to disk/stdout through a server-side cursor (`DECLARE … FETCH` in batches). Not bounded by `query_limit`, chunked under the statement timeout, and never routed through the ASCII renderer — the supported answer to "dump this table". `--format sql` emits `INSERT` statements (null-aware, type-aware quoting) and requires a table target. Read-only; never exposed via MCP.
- **Configurable `statement_timeout`**: `[settings] statement_timeout` (default `30s`) and a global `--timeout <duration>` flag (accepts `ms`, `s`, `min`, `h`; `0`/`disable` turns it off). Applies to read-only, MCP, and `exec` connections. Values are normalized by `config::normalize_statement_timeout` before interpolation, so only digits and a known unit reach `SET statement_timeout`.
- **Optional connection `schema`**: a per-connection `schema` key in `databases.ini` sets the connection `search_path` (single name or comma-separated, e.g. `analytics, public`) so unqualified table names resolve against it on every path (`query`, `sample`, `schema`, `exec`, and MCP). Values are normalized by `config::normalize_search_path` into double-quoted identifiers before reaching `SET search_path`, so only valid identifiers are interpolated; an invalid value fails the connect with `InvalidSearchPath`. Surfaced in the MCP `list_configured_databases` output.

## [0.1.8] - 2026-05-13

### Added

- **`WITH ... DML` chains in `exec`**: a leading `WITH` that resolves to an outer `INSERT`/`UPDATE`/`DELETE` is now accepted by `databasecli exec`. The resolver walks the comment-stripped analysis copy and picks the **most severe** verb across every CTE body and the outer DML, so `WITH d AS (DELETE FROM t RETURNING id) INSERT INTO log SELECT id FROM d` correctly classifies as `Destructive` and prompts for confirmation; the command tag uses the outer verb (`INSERT`) to match what PostgreSQL itself reports.
- **`databasecli exec --db <name> --file <PATH> [--transaction] [--yes]`**: run a multi-statement SQL script (seed files, migrations) on one configured database. The splitter is comment- and string-literal-aware and rejects dollar-quoted bodies up front. One short-lived writable connection runs the whole script so `BEGIN/COMMIT/SAVEPOINT` in the operator's SQL work as written. The destructive scan is global — one prompt lists every destructive statement with its source line number, bypassed by `--yes`. Non-interactive runs without `--yes` still return `ExecConfirmationRequired`.
- **`--transaction` flag (file mode only)**: wraps the script in an injected `BEGIN/COMMIT` pair for files that don't manage transactions themselves. Mutually exclusive with inline SQL.
- **Transaction-control verbs accepted as `Write`**: `BEGIN`, `COMMIT`, `ROLLBACK`, `START`, `END`, `SAVEPOINT`, `RELEASE`, `SET`, `RESET`, `LOCK`, `LISTEN`, `UNLISTEN`, `NOTIFY`, `DECLARE`, `FETCH`, `CLOSE`, `MOVE`, `CHECKPOINT`. No confirmation prompt; meaningful row counts are still suppressed for these tags.
- **`MERGE` classified as `Destructive`**: prompts unless `--yes` is set.
- **TUI bracketed paste + multi-line buffer**: the Execute screen now enables crossterm's bracketed-paste mode, so terminals deliver pasted multi-statement scripts as a single `Event::Paste(String)` instead of one `KeyCode::Char` per character. `execute_sql_buffer` is multi-line; `Enter` inserts a newline, `F5` runs, `Ctrl+R` runs as a fallback for terminals that drop function keys (notably tmux without `set -g xterm-keys on`). `c` clears the buffer in scroll mode.
- **Per-statement TUI results**: every script statement gets a `-- line N: <command tag>` header followed by the formatted rows/affected count, so a paste-and-run flow surfaces the result of each step.
- **TUI editor mode pill and cursor**: a `● TYPING` / `○ scroll` indicator in the editor header plus a bold green `▌` cursor make it obvious where the next keystroke will land.

### Changed

- **MCP read-only guard widened**: `crates/databasecli-mcp/tests/guard.rs::mcp_does_not_reference_writable_helpers` (renamed from `..._execute_statement`) now bans four symbols from every MCP source file: `execute_statement`, `execute_normalized`, `execute_script`, and `connect_for_local_exec`. An anchor comment documents that `ConnectionMode::LocalExec` is private, so any future writable connection mode requires a new public function in `databasecli-core` — and a matching ban entry in the same change.
- **`LiveConnection.client` is now `pub(crate)`**: command modules inside `databasecli-core` can still call `client.query`/`execute`/`prepare`/`batch_execute`, but dependent crates (`databasecli-mcp`, `databasecli-tui`, `databasecli-cli`) can no longer touch the raw `postgres::Client`. Server-side `default_transaction_read_only = on` remains the load-bearing guarantee; this is defense-in-depth so MCP source has no symbol-level path to `client.execute(...)`.
- **`has_returning_clause` is paren-depth-aware**: a `RETURNING` token inside a CTE body no longer trips the outer non-RETURNING DML into the prepare/query path. Only `RETURNING` at paren depth 0 of the comment-stripped analysis copy is detected.
- **`exec --file` errors carry line numbers**: per-statement runtime failures are wrapped as `line N: <error>`; chunk-level validation errors prepend `line N:` so the operator can find the offending line in the source file. Injected `BEGIN`/`COMMIT` from `--transaction` skip the annotation so synthetic lines are never reported.
- **Comment-only chunks silently skipped**: `INSERT...;\n-- trailing comment` parses as one statement, not a statement followed by an `EmptyQuery` error — matches `psql -f` behaviour. The whole-script-empty case is still rejected with `EmptyQuery`.
- **TUI Execute state types**: `AppAction::ExecuteStatement { database, sql }` became `AppAction::ExecuteScript { database, statements: Vec<ScriptStatement> }`; `BackgroundResult::Execute` now carries `Vec<ExecuteResult>`; `AppState::execute_result: Option<ExecuteResult>` was replaced with `execute_results: Vec<ExecuteResult>` plus `execute_pending_statements` and `execute_destructive_items`.
- **CLI help output**: `databasecli exec --help` and `databasecli reference` describe both inline and `--file` forms separately, including the remaining ban on procedural bodies, dollar-quoted strings, and `COPY`.

### Fixed

- **Nested-`WITH` under-classification**: `WITH outer AS (WITH inner AS (DELETE FROM t RETURNING id) SELECT * FROM inner) INSERT INTO log SELECT * FROM outer` now resolves to `Destructive` and triggers the confirmation prompt. Previously the inner `DELETE` sat at paren depth 2 and was silently ignored. The CTE-body capture rule now matches verbs at any paren depth `>= 1` immediately after `(`, not only depth 1.
- **Destructive subqueries in CTE bodies** are conservatively over-captured: `WITH d AS (SELECT 1 WHERE EXISTS (DELETE FROM other RETURNING id)) INSERT INTO log SELECT 1 FROM d` is treated as `Destructive` even though PostgreSQL would itself reject this exact shape. Safer direction — no destructive verb runs without a prompt.

### Security

- The MCP surface is unchanged and remains strictly read-only. Three layers protect it: `validate_readonly` (lexical, in `databasecli-core/src/commands/query.rs`), `default_transaction_read_only = on` (server-side, in `databasecli-core/src/connection.rs::open_client` ReadOnly mode), and the widened `mcp_does_not_reference_writable_helpers` guard test (in `databasecli-mcp/tests/guard.rs`). The new `pub(crate)` visibility on `LiveConnection.client` adds a compile-time wall — MCP source has neither a symbol path to a writable helper nor a field path to the raw `postgres::Client`.
- The bans on procedural bodies (`DO $$ ... $$`, `CREATE FUNCTION ... AS $body$ ... $body$`), dollar-quoted strings, and `COPY` apply to both inline `exec "..."` and `exec --file` paths.

## [0.1.7] - 2026-05-03

### Added

- **Local SQL execution (`exec`)**: New CLI subcommand `databasecli exec --db <name> [--yes] "<SQL>"` and matching TUI screen run a single write/DDL statement against one configured database. Uses a fresh, short-lived writable connection (still capped at `statement_timeout = '30s'`) — read-only sessions held by `ConnectionManager` are never mutated.
- **Destructive-statement confirmation**: `UPDATE`, `DELETE`, `DROP`, `TRUNCATE`, and `ALTER` prompt `Execute <KIND> on <db>? [y/N]` in the CLI; `--yes` bypasses. The TUI shows an explicit `Confirm` phase between SQL entry and execution. Non-interactive CLI runs without `--yes` fail fast instead of blocking.
- **TUI `Execute` screen**: Phase machine — `PickDatabase` (skipped when one DB is connected, otherwise picker over connected names) → `EditSql` → `Confirm` (destructive only) → `Result`. Esc cancels; non-destructive writes run immediately.
- **MCP read-only guards (tests)**: New `databasecli-mcp/tests/guard.rs` enforces that the MCP source never references `execute_statement`, no MCP tool function name implies writes, and `validate_readonly` keeps rejecting `INSERT/UPDATE/DELETE/DROP/TRUNCATE/ALTER/CREATE/GRANT` plus multi-statement smuggling.

### Changed

- **MCP `get_info()` instructions**: Server description now spells out that writes (`INSERT/UPDATE/DELETE/DROP/TRUNCATE/ALTER/CREATE/GRANT`, etc.) are unavailable to MCP clients by design and only reachable via the local `databasecli exec` operator path.

### Security

- `exec` v1 deliberately accepts a narrow SQL subset: single statement only, optional trailing semicolon, no `WITH` (writable CTEs cannot be classified safely), no procedural bodies (`DO $$ ... $$`, function definitions). Anything else is rejected before execution. The MCP surface is unchanged and remains strictly read-only — the new write path is local-only by construction and verified by guard tests.

## [0.1.6] - 2026-04-16

### Added

- **Multi-agent MCP configuration**: The `init` command now asks which coding agents to configure MCP for, instead of always writing `.mcp.json` only. Supported agents:
  - **Claude Code** (`.mcp.json`) — existing behavior, unchanged format
  - **Opencode** (`opencode.jsonc`) — `mcp.databasecli` entry with `type: "local"`, `command` array, and `enabled: true`
  - **Codex** (`.codex/config.toml`) — `[mcp_servers.databasecli]` table with `command` and `args`
  - **Cursor** (`.cursor/mcp.json`) — same `mcpServers` format as Claude Code
- **CLI agent selection prompt**: `databasecli init` displays a numbered list and accepts space-separated numbers (e.g. `1 3 4`). Unrecognized input prints a warning.
- **TUI agent selection screen**: The "Initialize Project" screen now shows a checkbox list with Space to toggle, j/k to navigate, and Enter to confirm. Selection resets each time the screen is opened.
- **JSONC comment support**: Existing `opencode.jsonc` files with `//` line comments or `/* */` block comments are parsed correctly. Comments inside JSON strings (e.g. URLs) are preserved.

### Changed

- **`run_init` accepts agent list**: The core `run_init()` function now takes a `&[CodingAgent]` parameter. Passing an empty slice creates only the config file without any MCP configuration.
- **`InitResult` reports per-agent results**: Replaced the single `mcp_path`/`mcp_action` fields with `agent_results: Vec<AgentInitResult>`, each containing the agent type, file path, and action taken.
- **Menu item description updated**: "Initialize Project" now reads "Create config and configure MCP for coding agents" instead of the previous `.mcp.json`-specific wording.

### Dependencies

- Added `toml` 0.8 to `databasecli-core` for Codex config file parsing and writing.

## [0.1.5] - 2026-04-03

### Added

- **Configurable query row limit**: New `[settings]` section in `databases.ini` with a `query_limit` option (default: 500). Caps the number of rows returned by user queries (`query`, `compare`) across CLI, TUI, and MCP. Set `query_limit = 0` to disable the limit.
- **Truncation indicator**: When results are capped, a clear notice is shown — yellow text in the TUI, a footer line in CLI output, and a `truncated` flag plus `truncation_notice` with pagination guidance in MCP JSON responses.
- **MCP pagination guidance**: Truncated MCP responses now include a `truncation_notice` field explaining the limit and suggesting `LIMIT`/`OFFSET` SQL pagination for AI agents.

### Changed

- **`establish_connections` returns settings**: CLI connection setup now loads settings in a single INI parse pass, eliminating redundant file reads for `query` and `compare` subcommands.
- **`compare_query` no longer double-validates SQL**: Removed the redundant `validate_readonly` call since `execute_query` already performs validation.

## [0.1.4] - 2026-04-01

### Fixed

- **Outdated config path in docs and UI**: Updated all references from `~/.databasecli/databases.ini` to the CWD-relative `.databasecli/databases.ini` to match the v0.1.3 config resolution change. Affected: website examples, README, privacy policy, MCP docs, TUI empty-state hints, and help screen.

## [0.1.3] - 2026-04-01

### Changed

- **Project-local config by default**: The config file is now resolved from the current working directory (`<cwd>/.databasecli/databases.ini`) instead of the home directory or exe directory. This means each project can have its own set of database connections. The `-D` flag, tilde expansion, and `DATABASECLI_CONFIG_PATH` env var overrides continue to work as before.

### Removed

- **Debug/release path split**: The previous behavior that resolved config from `target/debug/databases-dev.ini` in debug builds and `~/.databasecli/databases.ini` in release builds has been removed. Both modes now use the same cwd-based path.

## [0.1.2] - 2026-03-27

### Added

- **`init` command**: New `databasecli init [-D <path>]` subcommand that bootstraps a project in one step — creates `databases.ini` template if missing and creates or updates `.mcp.json` with the databasecli MCP server entry. Idempotent: safe to run multiple times. Available as both a CLI subcommand and a TUI menu item ("Initialize Project").
- **`databasecli-mcp` in releases**: The MCP server binary is now included in cargo-dist releases alongside the main CLI. Install via `brew install databasecli-mcp`, the shell/PowerShell installers, or MSI.
- **Tilde expansion in `-D` flag**: Paths like `~/projects/myapp` are now correctly expanded to the user's home directory. Previously, MCP clients that invoke binaries without a shell would pass `~` as a literal character, causing config resolution to fail.
- **Cross-platform install instructions**: README now documents installation for macOS (Homebrew), Linux (shell installer), Windows (PowerShell + MSI), and from source.

### Changed

- **`list` and `health` commands now respect `-D`**: Previously these two subcommands ignored the `-D` directory flag and always used the default config path. They now resolve config relative to the specified directory, consistent with all other subcommands.
- **`init` replaces `databasecli-mcp --init`**: The `--init` flag has been removed from the MCP binary. Use `databasecli init` instead, which also handles `.mcp.json` setup.
- **`FileAction` enum replaces boolean flags**: Init results now report `Created`, `Updated`, or `Unchanged` per file, giving accurate user feedback (e.g., "already configured" on no-op instead of misleading "updated").
- **Shared tilde expansion**: Extracted `expand_tilde()` and `resolve_base_dir()` helpers in `config.rs`, eliminating duplicated path expansion logic between config resolution and init.
- **TUI "Initialize Project" removes stale menu item**: When init creates the config file, the conditional "Create database.ini" menu item is removed from the home screen.
- **TUI init screen shows resolved paths**: Both the config path and `.mcp.json` path are displayed as fully resolved absolute paths, not raw `-D` input.
- **Updated help reference**: `databasecli reference` now lists the `init` command and points MCP init instructions to `databasecli init` instead of the removed `--init` flag.

## [0.1.0] - 2026-03-27

### Added

- **Full-screen TUI**: Interactive database management with screens for connection list, health monitoring, schema browsing, query execution, table analysis, ERD viewer, and inline help. Background health checks via `mpsc::channel` with non-blocking 100ms event loop polling.
- **CLI subcommands**: `list`, `health-check`, `schema`, `query`, `analyze`, `summary`, `erd`, `compare`, `trend`, `sample`, and `reference` — all scriptable with `--db` and `--all` flags for database targeting.
- **MCP server** (`databasecli-mcp`): Read-only MCP server over stdio exposing 14 tools for AI agent access — connection management, SQL querying, schema inspection, table analysis, ERD generation, time-series trends, and migration planning context. Compatible with Claude Desktop, Claude Code, and any stdio MCP client.
- **Read-only SQL enforcement**: Two-layer protection — server-side `SET default_transaction_read_only = on` on every connection, plus client-side SQL validation allowing only SELECT, WITH, EXPLAIN, SHOW, and TABLE statements. Multi-statement queries rejected. 30-second statement timeout.
- **INI-based configuration**: Database connections defined in `~/.databasecli/databases.ini` with per-section host, port, user, password, and dbname. `DATABASECLI_CONFIG_PATH` env var and `-D` flag for path overrides. `--init` flag on MCP server creates template config.
- **Multi-database support**: `--db <name>` flag (repeatable) to target specific databases, `--all` flag to connect to every configured database. `compare` subcommand runs the same query across all connected databases.
- **ERD generation**: Entity-relationship diagrams in ASCII, Mermaid, and DOT formats via `erd` subcommand with `--format` and `--output` flags.
- **Table profiling**: `analyze` subcommand inspects column nullability, cardinality, value distributions, and top values for any table.
- **Time-series analysis**: `trend` subcommand groups rows by day, week, month, or year on a timestamp column with optional value aggregation.
- **TLS connections**: All PostgreSQL connections use TLS encryption via `native-tls`.
- **Cross-platform**: macOS (ARM + Intel), Linux (x86_64), and Windows (x86_64) with platform-specific installers.

### Infrastructure

- Rust workspace with 4 crates: `databasecli-cli`, `databasecli-core`, `databasecli-tui`, `databasecli-mcp`
- `cargo-dist` v0.31.0 release pipeline with shell, PowerShell, Homebrew, and MSI installers
- Homebrew formula auto-published to `maximgorbatyuk/homebrew-tap` on release
- GitHub Actions CI: format check, clippy, build, and test on Linux and Windows
- Automated release script (`scripts/release.py`): version bump, verification, dev-to-main merge, tag push
