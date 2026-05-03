# SQL Execution Feature — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a local-only SQL execution path for CLI and TUI operators while preserving the MCP server's strict read-only contract.

**Architecture:** Keep all existing MCP and read-only query flows unchanged. Add a separate execution path that validates a narrower v1 SQL subset, opens short-lived local read-write connections from config on demand, and requires explicit confirmation before destructive operations.

**Tech Stack:** Rust 2024, `clap`, synchronous `postgres`, `crossterm`/`ratatui`, existing `databasecli-core` config and connection modules.

---

## Non-goals

- No new MCP tool, wrapper, or hidden code path for write execution.
- No change to `validate_readonly` or `execute_query` in `crates/databasecli-core/src/commands/query.rs`.
- No multi-statement support, transaction wrappers, or fan-out execution across multiple databases.
- No support in v1 for procedural bodies such as `DO $$ ... $$`, function definitions with embedded top-level semicolons, or other parser-heavy SQL forms.
- No change to existing read-only TUI/CLI query behavior or `query_limit` semantics.

## Prerequisite Decisions

### 1. Writable connections are separate from existing read-only sessions

The current connection path in `crates/databasecli-core/src/connection.rs` always sets `default_transaction_read_only = on`. That behavior must remain the default for MCP, `query`, `compare`, and all existing read-only screens.

`exec` must not reuse those read-only handles for actual execution. Instead:

- CLI `exec` resolves the selected database from config and opens a short-lived local read-write connection for that statement only.
- TUI `Execute` uses the selected connected database name only as a picker; when the user confirms execution, it resolves the database config and opens a short-lived local read-write connection on the background worker.
- The write-capable path still sets `statement_timeout = '30s'`, but does not set `default_transaction_read_only = on`.

This keeps the existing `ConnectionManager` behavior intact while making the write path explicit and local-only.

### 2. `exec` v1 supports a deliberately narrow SQL subset

The current semicolon helper in `query.rs` is too blunt for write execution because it rejects any semicolon outside single quotes. `exec` needs its own validator.

Lock in this v1 scope:

- Allow exactly one top-level statement.
- Allow a single optional trailing semicolon.
- Reject empty input.
- Reject procedural bodies and parser-heavy statement forms that require tracking nested/dollar-quoted SQL.
- Reject `WITH` in `exec` v1 rather than guessing whether the CTE is read-only or mutating.

This avoids false safety guarantees around writable CTEs and complex DDL bodies.

The validator only models SQL-standard string literals (`''` for an embedded apostrophe). PostgreSQL's `E'...'` *escape strings* (where `\'` is an escape) are not modelled, so an input that relies on `\'` may be rejected as multi-statement when PG would accept it. This is the safe direction — the validator is stricter than PG, never looser. Affected statements should be rewritten using `''` doubling.

### 3. `exec` is for local execution, not for read-only querying

To keep the operator experience clear:

- Pure read-only statements (`SELECT`, `SHOW`, `EXPLAIN`, `TABLE`) should be rejected by `exec` with a message directing the user to `query`.
- Statements in the supported write/DDL subset proceed through execution.
- Statements in ambiguous or unsupported forms fail fast with an explicit error instead of falling through to a misleading confirmation prompt.

### 4. Confirmation rules must be explicit

- `INSERT`, `CREATE`, `GRANT`, `VACUUM`, and similar non-destructive writes run without confirmation.
- `UPDATE`, `DELETE`, `DROP`, `TRUNCATE`, and `ALTER` require confirmation.
- CLI destructive execution prompts `Execute <KIND> on <db>? [y/N]`.
- If `stdin` is not a TTY and `--yes` was not supplied, CLI `exec` fails immediately instead of blocking on input.
- TUI destructive execution requires an explicit confirmation state. `Esc` cancels.

## Core Changes

### New connection helper(s)

In `crates/databasecli-core/src/connection.rs`:

- Keep the existing read-only connection behavior as the default path used by `ConnectionManager::connect`.
- Add a helper for one-shot local execution connections, such as `connect_for_local_exec(config) -> LiveConnection`, or introduce an internal `ConnectionMode` helper that preserves the existing public API while enabling a separate read-write branch.
- Ensure the local exec path still sets `statement_timeout = '30s'`.

Do not weaken the current read-only connect path used by MCP and query flows.

### New module: `crates/databasecli-core/src/commands/execute.rs`

Add a dedicated execution module instead of overloading `query.rs`.

Expected responsibilities:

- `validate_single_statement(sql) -> Result<NormalizedStatement, DatabaseCliError>`
  - Trims whitespace.
  - Allows one optional trailing semicolon.
  - Rejects multiple top-level statements.
  - Rejects unsupported parser-heavy forms for v1.
- `classify_statement(sql) -> StatementKind`
  - `Read`
  - `Write`
  - `Destructive`
  - `Unsupported`
- `execute_statement(conn, sql) -> ExecuteResult`
  - Does not apply `query_limit`.
  - Uses `query` for statements expected to return rows (`... RETURNING ...`).
  - Uses `simple_query` for DDL / command-tag-oriented statements.
  - Returns `command_tag`, `affected_rows`, optional `columns`, optional `rows`, and `execution_time`.
- `format_execute_result(&ExecuteResult) -> String`
  - Formats row-returning results similarly to `format_query_result`.
  - Formats command-tag-only results cleanly for DDL / non-returning DML.

Add a doc comment to `execute_statement`:

`/// Local CLI/TUI execution only. Do NOT expose through databasecli-mcp.`

### Error handling

In `crates/databasecli-core/src/error.rs`:

- Reuse `QueryFailed` if the wording still reads naturally.
- Add targeted variants only where they improve operator clarity, for example:
  - `UnsupportedExecStatement(String)`
  - `ExecConfirmationRequired`
  - `ExecCancelled`

Do not add a new error enum just for `exec`.

## CLI Changes

### Command surface

In `crates/databasecli-cli/src/args.rs`:

- Add `Exec`:
  - positional `sql: String`
  - `--yes` to bypass destructive confirmation

In `crates/databasecli-cli/src/main.rs` and `run.rs`:

- Add `run_exec(&Cli, &str, yes: bool)`.

### Selection rules

`exec` must enforce a stricter database-selection contract than read-only commands:

- Exactly one `--db <name>` is required.
- `--all` is rejected.
- Repeated `--db` values resulting in multiple targets are rejected.
- Zero selected databases is rejected.

The plan should implement this validation in `run_exec` even if `clap` permits the global flags syntactically.

### Execution flow

`run_exec` should:

1. Resolve the one selected database config.
2. Validate and classify the SQL with the new `execute` module.
3. Reject `Read` statements with guidance to use `query`.
4. If destructive and `--yes` is not set:
   - require an interactive TTY
   - prompt on stdin/stdout
   - cancel cleanly on anything except explicit `y` / `Y`
5. Open a short-lived local exec connection for the selected database.
6. Execute the statement and print the formatted result.

### CLI cancellation behavior

Define this explicitly in the implementation:

- User declines destructive confirmation: print `Cancelled.` and exit without executing.
- Non-interactive stdin without `--yes`: return an error that explains `--yes` is required outside interactive terminals.

## TUI Changes

### New screen and state machine

The current input-driven screens share a single `input_buffer` / `input_mode` flow. `Execute` needs its own explicit state, not just another clone of `Query`.

In `crates/databasecli-tui/src/app.rs`:

- Add `MenuItem::Execute` and `Screen::Execute`.
- Add `AppAction::ExecuteStatement { database: String, sql: String }`.
- Add dedicated execute state, for example:
  - selected database cursor / name
  - phase enum such as `PickDatabase | EditSql | Confirm | Result`
  - pending `StatementKind`
  - last `ExecuteResult`

Do not overload `query_result` or rely only on the generic `input_mode` boolean.

### Database selection rules

For TUI `Execute`:

- If exactly one database is connected, auto-select its name and skip to SQL entry.
- If 2+ databases are connected, render a picker over `connected_names`.
- If none are connected, keep the existing "Connect to a database first" behavior.

Important: the selected connected name is only a UI selection key. The actual write execution must resolve that name back to config and open a fresh local exec connection.

### Event handling

In `crates/databasecli-tui/src/event/`:

- Add explicit handling for execute phases instead of routing everything through the generic input-screen handler.
- Support:
  - database picker navigation (`j` / `k`, Enter, Esc)
  - SQL entry mode
  - destructive confirmation (`y`, `n`, `Esc`)
  - scroll / result viewing after execution

### Background execution path

In `crates/databasecli-tui/src/lib.rs`:

- Add a new `BackgroundResult::Execute(Result<ExecuteResult, String>)`.
- Do not run execution against `manager.iter_mut().next()` like the current `RunQuery` path.
- On the worker thread:
  - resolve the selected database config by name
  - open a one-shot local exec connection
  - call `execute_statement`
- Render results through a new `ui/execute.rs`.

This preserves the current read-only connected sessions and avoids trying to mutate through a read-only connection.

## MCP Guardrails

The read-only MCP guarantee must be enforced by behavior, not just by omission.

### Keep the MCP tool surface unchanged

In `crates/databasecli-mcp/src/server.rs` and `crates/databasecli-mcp/src/tools/`:

- Do not add an `exec` tool.
- Do not add wrappers around `execute_statement`.
- Keep MCP `query` and `compare` on `execute_query`.

### Strengthen tests instead of relying on a fragile grep

Replace the current source-grep idea with stronger guards:

- Add a test that the exposed MCP tool list does not include `exec` / `execute`.
- Add or keep a test proving MCP `query` still rejects write SQL.
- Update `get_info()` instructions to state that write operations are unavailable to MCP clients by design and only reachable through the local CLI/TUI operator path.

If a source grep is still added, treat it as a secondary guard only.

## Tests

Split testing into unit, state-machine, and opt-in live database coverage.

### Core unit tests

In `crates/databasecli-core/src/commands/execute.rs` tests:

- empty input rejected
- single trailing semicolon allowed
- multi-statement input rejected
- read statements rejected from `exec`
- write statements classified correctly
- destructive statements classified correctly
- `WITH` rejected as unsupported in v1
- comments hiding the first keyword handled correctly
- unsupported procedural forms rejected

Also unit-test result formatting for:

- command-tag-only output
- row-returning output
- affected-rows reporting

### CLI tests

In `crates/databasecli-cli/` tests:

- `exec` rejects `--all`
- `exec` rejects zero `--db`
- `exec` rejects multiple `--db`
- `exec --yes` bypasses destructive prompt
- non-interactive destructive exec without `--yes` errors clearly

### TUI state tests

Add focused state-machine tests around `AppState` or a dedicated execute-state module:

- one connected DB skips picker
- multiple connected DBs enter picker
- destructive statement enters confirm phase
- `Esc` cancels from confirm phase
- execute action includes selected database name and SQL

### MCP regression tests

In `crates/databasecli-mcp/` tests:

- exposed tool list contains no write-execution tool
- MCP read-only query path still rejects `INSERT` / `UPDATE` / `DELETE`

### Opt-in live database verification

Keep live write execution out of the default suite, but define an explicit opt-in integration or manual verification path that proves:

- local `exec` can perform a safe test write against a disposable database
- read-only `query` still fails for the same write statement
- MCP still cannot execute the same write statement

## Docs

Update all operator-facing documentation, not just `README.md`.

### Files to update

- `CHANGELOG.md`
- `README.md`
- `docs/mcp.md`
- `crates/databasecli-core/src/help.rs`
- TUI help text / menu descriptions if needed

### Documentation requirements

- Document `databasecli exec "<SQL>" --db <name> [--yes]`
- Explain that `exec` is local-only and intentionally absent from MCP
- Document destructive confirmation behavior
- Document the v1 SQL limitations:
  - no multi-statement execution
  - no `WITH` in `exec`
  - no procedural bodies / transaction scripts
- Keep the built-in help/reference aligned with the new command surface and the existing MCP security story

## Manual Verification Checklist

Before calling the feature complete, verify all of the following on a disposable database:

1. `databasecli --db testdb query "INSERT INTO ..."` still fails as read-only.
2. `databasecli --db testdb exec "INSERT INTO ..."` succeeds.
3. `databasecli --db testdb exec "DELETE FROM ..."` prompts.
4. `databasecli --db testdb exec --yes "DELETE FROM ..."` skips the prompt.
5. `databasecli --db testdb exec "SELECT 1"` is rejected with guidance to use `query`.
6. TUI `Execute` uses the chosen database, not the first connected database.
7. MCP tool list remains unchanged.
8. MCP `query` still rejects write SQL.

## Behaviors Locked In

- MCP remains read-only by design and by test coverage.
- Existing read-only connections remain read-only.
- Local write execution always uses a separate short-lived connection.
- `exec` v1 is intentionally narrow: single supported statement only, optional trailing semicolon, no `WITH`, no procedural bodies.
- CLI `exec` requires exactly one `--db` and never supports `--all`.
- CLI destructive execution prompts unless `--yes` is supplied.
- TUI destructive execution requires explicit confirmation.
- `query_limit` never applies to `exec`.
