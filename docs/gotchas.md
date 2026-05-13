# Gotchas

Non-obvious constraints that bite when you forget about them. Each entry names the source of truth so the reader can verify the current state.

## TLS does not verify the server certificate

`open_client` and `check_health` build the TLS connector with `native_tls::TlsConnector::builder().danger_accept_invalid_certs(true)`. Connections are encrypted but not validated against any CA. This is fine for local/dev databases and intentionally permissive for the v1 surface, but it is **not** a substitute for proper certificate validation on untrusted networks. If you tighten this, update [`./mcp.md`](./mcp.md) and the Security section of `README.md`. Source: [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs), [`crates/databasecli-core/src/health.rs`](../crates/databasecli-core/src/health.rs).

## crossterm fires both Press and Release on Windows

On Windows, every keystroke generates a `KeyEventKind::Press` event followed by a `KeyEventKind::Release` event. The TUI event router filters with `if key.kind != KeyEventKind::Press { return; }` at the top of `handle_key`. New TUI key handlers must rely on this filter; missing it causes every keystroke to fire twice. Source: [`crates/databasecli-tui/src/event/mod.rs`](../crates/databasecli-tui/src/event/mod.rs).

## TUI normalises Cyrillic ЙЦУКЕН keys to QWERTY — except in input mode

The TUI assumes single-letter shortcuts like `q`, `j`, `k`, `i`. To stay usable on Russian keyboard layouts, the event router transparently maps Cyrillic letters to their QWERTY positions (`й → q`, `ц → w`, ...). The mapping is **disabled** while any text-input buffer is active (`AppState::is_typing()`), so SQL typed in Cyrillic is preserved verbatim. Adding a new screen with its own input buffer requires extending `is_typing()` so this rule keeps holding. Source: [`crates/databasecli-tui/src/event/mod.rs`](../crates/databasecli-tui/src/event/mod.rs), [`crates/databasecli-tui/src/app.rs`](../crates/databasecli-tui/src/app.rs).

## MCP cannot run writes — and the build enforces it

`crates/databasecli-mcp/tests/guard.rs` is more than a smoke test. It (1) refuses to compile the test suite if any MCP source string contains any of the banned writable-helper symbols (`execute_statement`, `execute_normalized`, `execute_script`, `connect_for_local_exec`), (2) refuses if any top-level `fn` in `server.rs` is not on a hand-maintained allowlist, (3) refuses if `validate_readonly` ever stops rejecting common write verbs or multi-statement smuggling. The intended workflow when adding an MCP tool is described in [`./interactions.md`](./interactions.md) — skip a step and the test fails the build, not just the suite. Source: [`crates/databasecli-mcp/tests/guard.rs`](../crates/databasecli-mcp/tests/guard.rs).

## The `exec` validator is stricter than PostgreSQL on purpose

`validate_single_statement` only models SQL-standard string literals (doubled `''` for an embedded apostrophe). PostgreSQL's `E'...'` *escape strings* (where `\'` is an escape character) are **not** modelled, so an input like `INSERT INTO t VALUES (E'a\';DROP TABLE t;--')` is over-rejected as multi-statement. This is the safe direction. Operators must rewrite affected statements with `''` doubling. Source: [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `exec` runs the operator's original SQL, not the analysis copy

`validate_single_statement` strips comments to a separate analysis copy used only for classification (first-keyword extraction, `RETURNING` detection, multi-statement scan). The executable string is built from the *original* operator input with only surrounding whitespace trimmed and at most one trailing semicolon removed. This invariant prevents an unterminated `/*` from silently widening a destructive statement. If you change either path, keep the executable string close to the original — the safety guarantee depends on it. Source: code comments in [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `exec` accepts `WITH ... DML` via a heuristic, not a real parser

`exec` accepts a leading `WITH` when the chain resolves to a top-level DML statement (e.g. `WITH d AS (...) INSERT INTO ...`). `resolve_with_kind` walks the comment-stripped analysis copy, collects every classified verb that appears at paren depth 1 immediately after `(` (each CTE body's leading verb) plus the first classified verb at depth 0 after the first descent (the outer DML), and picks the **most severe** kind across them. The outer verb drives the command tag; severity drives the confirmation prompt. Mixed chains like `WITH d AS (DELETE...) INSERT ...` are classified `Destructive` so the operator is prompted, even though the outer is INSERT.

Caveats: the heuristic is precision-loss tolerant. Pathological inputs that aren't valid PostgreSQL (e.g. `DELETE` nested inside a non-CTE subquery) may be under-classified, but they would fail at PostgreSQL parse time anyway. `WITH RECURSIVE`, `VALUES` CTE bodies, and INSERT/SELECT-source patterns all classify correctly. `WITH ... SELECT` chains are routed to the read-only path and rejected by `exec` with the standard "use query" message. Source: `resolve_with_kind` in [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `exec` still rejects procedural bodies, dollar-quoted strings, and `COPY`

`exec` rejects `DO $$ ... $$`, `CREATE FUNCTION ... AS $body$ ... $body$`, and any other dollar-quoted body — the validator does not model `$...$` escaping and cannot safely classify the inner body. `COPY` is rejected separately because the `exec` path does not implement the `copy_in`/`copy_out` streaming APIs that `COPY` requires; treating it as a regular write would either drop the data stream or produce a confusing protocol error. These bans apply to both inline `exec "..."` and `exec --file <PATH>`. Source: `contains_dollar_quote`, `classify_keyword`, and `split_script` in [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `exec --file` runs every statement on a single writable connection

When `exec --file <PATH>` is invoked, the splitter divides the file into single statements (string-literal and comment aware, dollar-quote rejecting) and a single `connect_for_local_exec` connection runs them in order. This is the deliberate exception to "open a fresh connection per call" — running BEGIN/COMMIT/SAVEPOINT across multiple connections would deadlock or break atomicity. The single-connection lifetime is bounded by one invocation; the connection is dropped immediately after the script finishes (success or first error). `--transaction` wraps the script in an injected BEGIN/COMMIT pair when the operator's file does not manage transactions itself. Source: `run_exec_file` in [`crates/databasecli-cli/src/run.rs`](../crates/databasecli-cli/src/run.rs), `split_script` and `execute_script` in [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `query_limit = 0` means unlimited

The `[settings] query_limit` value defaults to 500. Setting it to `0` disables the wrapper LIMIT for `query`/`compare`/MCP `query`. Any positive value caps the result set; the wrapper requests one extra row to detect truncation, and the `truncated` field on `QueryResultSet` indicates whether more rows existed. Source: [`crates/databasecli-core/src/config.rs`](../crates/databasecli-core/src/config.rs), [`crates/databasecli-core/src/commands/query.rs`](../crates/databasecli-core/src/commands/query.rs).

## Connection timeout is 5 s, statement timeout is 30 s

`DatabaseConfig::connection_string` always appends `connect_timeout=5`, and every code path (read-only and exec) sets `statement_timeout = '30s'` after connecting. There is no per-database override and no way to extend the budget for a long-running query. Long-running operational queries should be split or pre-aggregated. Source: [`crates/databasecli-core/src/config.rs`](../crates/databasecli-core/src/config.rs), [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs).

## Local `exec` opens a fresh connection per invocation

Every `exec` invocation goes through `connect_for_local_exec`, which opens a brand new `postgres::Client` without `default_transaction_read_only = on`. The read-only sessions held by `ConnectionManager` are never "promoted" to writable. Inline `exec "..."` and the TUI's Execute screen both pay one connect per run. `exec --file` reuses that single fresh connection for every statement in the script (see the dedicated entry above) so BEGIN/COMMIT/SAVEPOINT work as written, but the connection still belongs to that one invocation and is dropped when the script ends. Source: [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs), [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `exec` requires exactly one `--db`

`databasecli exec` rejects `--all` and rejects more than one `--db <name>`. The TUI `Execute` screen has the same constraint — it skips the picker only when one database is connected. Multi-database write fan-out is intentionally not supported in v1. Source: [`crates/databasecli-cli/src/run.rs`](../crates/databasecli-cli/src/run.rs), [`crates/databasecli-tui/src/app.rs`](../crates/databasecli-tui/src/app.rs).

## Destructive `exec` fails fast in non-interactive shells

When `stdin` is not a TTY and any statement in the run is destructive (`UPDATE`, `DELETE`, `DROP`, `TRUNCATE`, `ALTER`, `MERGE`, or a `WITH` chain that contains any of those), `run_exec` returns `DatabaseCliError::ExecConfirmationRequired` instead of blocking on a prompt that would never receive input. For `exec --file`, the destructive scan covers every statement in the script; the single prompt then lists each destructive line with its source line number. Pass `--yes` to bypass the prompt. Source: [`crates/databasecli-cli/src/run.rs`](../crates/databasecli-cli/src/run.rs), [`crates/databasecli-core/src/error.rs`](../crates/databasecli-core/src/error.rs).

## TUI Execute accepts pasted scripts and runs with F5 or Ctrl+R

The Execute screen enables crossterm's bracketed-paste mode, so terminals deliver pasted multi-statement text as a single `Event::Paste(String)` instead of one `KeyCode::Char` per character. `execute_sql_buffer` is a multi-line buffer: `Enter` inserts a newline, `F5` runs. `Ctrl+R` is an alternate run key for terminals that drop function keys (notably tmux without `set -g xterm-keys on`, plus a handful of older emulators). Both keys work in input mode and read mode. Read mode (Esc out of input mode) accepts `c` to clear the buffer. The buffer is split via `split_script` exactly as `exec --file` does — same multi-statement semantics, same destructive confirmation list, same connection lifetime. Source: [`crates/databasecli-tui/src/lib.rs`](../crates/databasecli-tui/src/lib.rs), [`crates/databasecli-tui/src/event/execute.rs`](../crates/databasecli-tui/src/event/execute.rs), [`crates/databasecli-tui/src/app.rs`](../crates/databasecli-tui/src/app.rs).

## `LiveConnection.client` is `pub(crate)`, not `pub`

The raw `postgres::Client` inside `LiveConnection` is visible to command modules in `databasecli-core` (which need it to call `query`/`execute`/`prepare`) but **not** to dependent crates. `databasecli-mcp`, `databasecli-tui`, and `databasecli-cli` can only act on a connection through a function exported from `databasecli-core`. Combined with the `mcp_does_not_reference_writable_helpers` guard test, this means MCP cannot reach `client.execute(...)` even by reflection — there is no symbol to call, and the field accessor doesn't compile from outside the core crate. Server-side `default_transaction_read_only = on` remains the load-bearing guarantee; this is defense-in-depth. Source: [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs).

## `init` is idempotent — but only on the *databasecli* MCP entry

`upsert_claude_code`, `upsert_opencode`, and `upsert_codex` all return `FileAction::Unchanged` when a `databasecli` entry already exists in the target file. They do **not** validate the surrounding structure or migrate older entries. If a file is hand-edited into an unparsable state, `init` will fail with `ConfigParse` — operators must fix the file or delete it. Re-running `init` on a working layout is safe and prints `Already configured`. Source: [`crates/databasecli-core/src/init.rs`](../crates/databasecli-core/src/init.rs).

## Config path is project-local, not global

The default INI location is `<cwd>/.databasecli/databases.ini`. There is no fallback to `$HOME/.databasecli/...`. `~` expansion only applies to `-D` paths via the `home` crate. The `DATABASECLI_CONFIG_PATH` env var takes priority over both `-D` and the cwd default. Tests rely on this priority order — see [`./testing.md`](./testing.md). Source: [`crates/databasecli-core/src/config.rs`](../crates/databasecli-core/src/config.rs).

## `suggest_migration` returns context, not DDL

The MCP tool described as a migration helper analyses schema and returns the dump plus the operator's description. It never executes DDL and never composes a migration. Agents are expected to generate migration SQL themselves and then apply it through the local `databasecli exec` path. Document this clearly in any tool-description change. Source: [`crates/databasecli-mcp/src/server.rs`](../crates/databasecli-mcp/src/server.rs), [`crates/databasecli-mcp/src/tools/migration.rs`](../crates/databasecli-mcp/src/tools/migration.rs).

## Passwords are stored in plaintext

INI sections store the database password verbatim. There is no keychain integration, no env-var indirection, and no secret manager hook. The expected mitigation is filesystem permissions (`chmod 600 .databasecli/databases.ini`) plus gitignoring the file in the consuming project. Document any future change here and in [`./mcp.md`](./mcp.md). Source: [`crates/databasecli-core/src/config.rs`](../crates/databasecli-core/src/config.rs).

## Releases must update `CHANGELOG.md` first

`scripts/release.py` validates that `CHANGELOG.md` already contains a section for the new version and that it is the latest entry — *before* it touches `Cargo.toml` or runs `just verify`. Forgetting to add the changelog entry aborts the release with a clear message. The script also auto-increments the patch number when run without an argument; pass `X.Y.Z` to bump minor or major. Source: [`scripts/release.py`](../scripts/release.py).
