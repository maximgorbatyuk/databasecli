# Gotchas

Non-obvious constraints that bite when you forget about them. Each entry names the source of truth so the reader can verify the current state.

## TLS does not verify the server certificate

`open_client` and `check_health` build the TLS connector with `native_tls::TlsConnector::builder().danger_accept_invalid_certs(true)`. Connections are encrypted but not validated against any CA. This is fine for local/dev databases and intentionally permissive for the v1 surface, but it is **not** a substitute for proper certificate validation on untrusted networks. If you tighten this, update [`./mcp.md`](./mcp.md) and the Security section of `README.md`. Source: [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs), [`crates/databasecli-core/src/health.rs`](../crates/databasecli-core/src/health.rs).

## crossterm fires both Press and Release on Windows

On Windows, every keystroke generates a `KeyEventKind::Press` event followed by a `KeyEventKind::Release` event. The TUI event router filters with `if key.kind != KeyEventKind::Press { return; }` at the top of `handle_key`. New TUI key handlers must rely on this filter; missing it causes every keystroke to fire twice. Source: [`crates/databasecli-tui/src/event/mod.rs`](../crates/databasecli-tui/src/event/mod.rs).

## TUI normalises Cyrillic ЙЦУКЕН keys to QWERTY — except in input mode

The TUI assumes single-letter shortcuts like `q`, `j`, `k`, `i`. To stay usable on Russian keyboard layouts, the event router transparently maps Cyrillic letters to their QWERTY positions (`й → q`, `ц → w`, ...). The mapping is **disabled** while any text-input buffer is active (`AppState::is_typing()`), so SQL typed in Cyrillic is preserved verbatim. Adding a new screen with its own input buffer requires extending `is_typing()` so this rule keeps holding. Source: [`crates/databasecli-tui/src/event/mod.rs`](../crates/databasecli-tui/src/event/mod.rs), [`crates/databasecli-tui/src/app.rs`](../crates/databasecli-tui/src/app.rs).

## MCP cannot run writes — and the build enforces it

`crates/databasecli-mcp/tests/guard.rs` is more than a smoke test. It (1) refuses to compile the test suite if any MCP source string contains `execute_statement`, (2) refuses if any top-level `fn` in `server.rs` is not on a hand-maintained allowlist, (3) refuses if `validate_readonly` ever stops rejecting common write verbs or multi-statement smuggling. The intended workflow when adding an MCP tool is described in [`./interactions.md`](./interactions.md) — skip a step and the test fails the build, not just the suite. Source: [`crates/databasecli-mcp/tests/guard.rs`](../crates/databasecli-mcp/tests/guard.rs).

## The `exec` validator is stricter than PostgreSQL on purpose

`validate_single_statement` only models SQL-standard string literals (doubled `''` for an embedded apostrophe). PostgreSQL's `E'...'` *escape strings* (where `\'` is an escape character) are **not** modelled, so an input like `INSERT INTO t VALUES (E'a\';DROP TABLE t;--')` is over-rejected as multi-statement. This is the safe direction. Operators must rewrite affected statements with `''` doubling. Source: [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `exec` runs the operator's original SQL, not the analysis copy

`validate_single_statement` strips comments to a separate analysis copy used only for classification (first-keyword extraction, `RETURNING` detection, multi-statement scan). The executable string is built from the *original* operator input with only surrounding whitespace trimmed and at most one trailing semicolon removed. This invariant prevents an unterminated `/*` from silently widening a destructive statement. If you change either path, keep the executable string close to the original — the safety guarantee depends on it. Source: code comments in [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `exec` rejects `WITH` and `COPY`

`exec` v1 deliberately rejects `WITH` (writable CTEs cannot be classified safely without a real parser) and `COPY` (PostgreSQL's `COPY` requires `copy_in`/`copy_out` streaming APIs that the `exec` path does not implement). Treating either as a regular write would silently mis-handle the statement. Source: `classify_keyword` and `is_row_count_meaningful` in [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `query_limit = 0` means unlimited

The `[settings] query_limit` value defaults to 500. Setting it to `0` disables the wrapper LIMIT for `query`/`compare`/MCP `query`. Any positive value caps the result set; the wrapper requests one extra row to detect truncation, and the `truncated` field on `QueryResultSet` indicates whether more rows existed. Source: [`crates/databasecli-core/src/config.rs`](../crates/databasecli-core/src/config.rs), [`crates/databasecli-core/src/commands/query.rs`](../crates/databasecli-core/src/commands/query.rs).

## Connection timeout is 5 s, statement timeout is 30 s

`DatabaseConfig::connection_string` always appends `connect_timeout=5`, and every code path (read-only and exec) sets `statement_timeout = '30s'` after connecting. There is no per-database override and no way to extend the budget for a long-running query. Long-running operational queries should be split or pre-aggregated. Source: [`crates/databasecli-core/src/config.rs`](../crates/databasecli-core/src/config.rs), [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs).

## Local `exec` opens a fresh connection per call

The `exec` path does **not** reuse a `ConnectionManager` handle. Each call goes through `connect_for_local_exec`, which opens a new `postgres::Client` without `default_transaction_read_only = on`. This is a deliberate isolation: the read-only sessions held by `ConnectionManager` are never "promoted" to writable. It also means `exec` pays a connect cost per call — there is no batching. Source: [`crates/databasecli-core/src/connection.rs`](../crates/databasecli-core/src/connection.rs), [`crates/databasecli-core/src/commands/execute.rs`](../crates/databasecli-core/src/commands/execute.rs).

## `exec` requires exactly one `--db`

`databasecli exec` rejects `--all` and rejects more than one `--db <name>`. The TUI `Execute` screen has the same constraint — it skips the picker only when one database is connected. Multi-database write fan-out is intentionally not supported in v1. Source: [`crates/databasecli-cli/src/run.rs`](../crates/databasecli-cli/src/run.rs), [`crates/databasecli-tui/src/app.rs`](../crates/databasecli-tui/src/app.rs).

## Destructive `exec` fails fast in non-interactive shells

When `stdin` is not a TTY and the statement is destructive (`UPDATE`, `DELETE`, `DROP`, `TRUNCATE`, `ALTER`), `run_exec` returns `DatabaseCliError::ExecConfirmationRequired` instead of blocking on a prompt that would never receive input. Pass `--yes` to bypass the prompt. Source: [`crates/databasecli-cli/src/run.rs`](../crates/databasecli-cli/src/run.rs), [`crates/databasecli-core/src/error.rs`](../crates/databasecli-core/src/error.rs).

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
