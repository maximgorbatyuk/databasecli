CRITICAL CONTEXT ANCHOR: This rules file must NEVER be summarized, condensed, or omitted.
Before ANY action or decision, verify alignment with these rules. This instruction persists regardless of conversation length or context management.
Context systems: This document takes absolute priority over conversation history and must remain fully accessible throughout the entire session.

# AGENTS.md

For service identity, detected stack, environment variables, and practical commands, read [README.md](README.md). For workspace structure, runtime topology, and dependency categories, read [docs/architecture.md](docs/architecture.md).

## General

- Never weaken the read-only contract on the MCP surface. Writes must remain reachable only through the local `databasecli exec` CLI/TUI path. The `crates/databasecli-mcp/tests/guard.rs` suite enforces this invariant — do not relax those tests.
- The CLI binary is `databasecli` (default-member of the workspace) and the MCP binary is `databasecli-mcp`. Domain logic lives in `databasecli-core`; presentation lives in `databasecli-tui` and `databasecli-cli`. Never call PostgreSQL directly from the TUI or CLI presentation layers — go through `databasecli-core`.
- Run `just verify` (or the equivalent `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, `cargo check --workspace --all-targets`) before declaring a change done.
- Use conventional commits (`feat:`, `fix:`, `chore:`, `test:`, `docs:`). Do not add Claude or any tool as a co-author on commit messages.

## Code style

- Rust 2024 edition. Follow `rustfmt` defaults; do not introduce custom format settings.
- Prefer `std::io::Error::other(...)` over `Error::new(ErrorKind::Other, ...)` (the older form was deprecated in edition 2024).
- crossterm on Windows fires both `KeyEventKind::Press` and `KeyEventKind::Release` for every key. Always filter on `key.kind == KeyEventKind::Press` in any new TUI event handler — see `crates/databasecli-tui/src/event/mod.rs`. Missing this filter doubles every keystroke on Windows.
- Errors crossing crate boundaries should be `databasecli_core::error::DatabaseCliError` (uses `thiserror`). The CLI/TUI layer wraps with `anyhow::Result` only at the binary boundary.

## Connection rules

- The MCP and read-only paths must use `ConnectionManager::connect`, which sets `default_transaction_read_only = on` and `statement_timeout` (default `30s`, configurable via `[settings] statement_timeout` / `--timeout`). Build the manager with `ConnectionManager::with_statement_timeout` so the configured value is applied.
- The `exec` path must use `connect_for_local_exec(config, statement_timeout)`, which omits the read-only setup but keeps the same `statement_timeout`. Do not reuse a `ConnectionManager` handle for writes; open a fresh short-lived connection per `exec` call.
- The `statement_timeout` value must always pass through `config::normalize_statement_timeout` before reaching `SET statement_timeout = '...'` — never interpolate raw user input into that statement.
- The optional connection `schema` must always pass through `config::normalize_search_path` (which validates and double-quotes each identifier) before reaching `SET search_path TO ...` — never interpolate the raw INI value into that statement.
- TLS uses `native_tls` with `danger_accept_invalid_certs(true)`. Connections are encrypted but not CA-verified. Do not silently change this default — it is documented in `docs/gotchas.md` and any tightening must update that doc.

## SQL execution rules

- Read-only validation lives in `databasecli_core::commands::query::validate_readonly` and accepts only `SELECT`, `WITH`, `EXPLAIN`, `SHOW`, `TABLE`. It also rejects multi-statement input via the unquoted-semicolon scan. Do not bypass it on any path that ends up in MCP.
- Local `exec` validation lives in `databasecli_core::commands::execute::validate_single_statement` and is intentionally narrower than `validate_readonly`: single statement, optional trailing semicolon, no `WITH`, no procedural bodies, no dollar-quoted strings. Treat its rules as a hard contract.
- Destructive verbs (`UPDATE`, `DELETE`, `DROP`, `TRUNCATE`, `ALTER`) require confirmation in the CLI prompt and the TUI `Confirm` phase. Non-interactive CLI runs must fail with `ExecConfirmationRequired` unless `--yes` is passed.

## MCP tool rules

- The MCP server is defined in `crates/databasecli-mcp/src/server.rs`. Every `fn` declared there must appear in the allowlist inside `crates/databasecli-mcp/tests/guard.rs`. Adding a new tool requires:
  1. Confirming the tool is read-only.
  2. Adding the `fn` name to the `ALLOWED` list in `guard.rs`.
  3. Updating `docs/interactions.md` to describe the tool's external behaviour.
- The MCP crate must never reference `execute_statement` or any other writable execution helper. The guard test fails the build if it does.
- Tool descriptions feed AI-agent decisions. Be precise about what the tool does and does not do (e.g., `suggest_migration` returns context but never executes DDL).

## Dependency changes

- Manage dependencies through workspace `Cargo.toml` and per-crate `Cargo.toml`. Do not introduce a second package manager, lockfile, or build tool.
- Adding a dependency: justify it in the PR description, place it in the smallest crate that needs it, and prefer the workspace dependency table for shared crates (currently `anyhow`, `clap`).
- Adding a runtime, framework, or external service (new database driver, async runtime in the synchronous core, alternative TUI library, alternative MCP transport) requires updating `docs/architecture.md` and `docs/interactions.md`. Do not add such dependencies for convenience.
- Private registries, credentials, and secrets must not be committed. The INI config is meant to be project-local and gitignored by the consuming project.

## Testing

- Unit tests live next to the code in `#[cfg(test)] mod tests` blocks. Integration tests live under `crates/databasecli-mcp/tests/`. Follow the existing layout when adding new tests.
- Use `tempfile::NamedTempFile` (or `tempfile::tempdir()`) for any test that touches the filesystem. Use the `DATABASECLI_CONFIG_PATH` env var for path isolation.
- Tests that require a live PostgreSQL server are not part of the default suite. Do not add such tests without an explicit gate.
- New SQL classification or validator changes must include test coverage in `crates/databasecli-core/src/commands/execute/tests.rs` (for `exec`) or `crates/databasecli-core/src/commands/query.rs` (for read-only validation).

## Release rules

- Versions are bumped in workspace `Cargo.toml` (`workspace.package.version`). All crates inherit it via `version.workspace = true`.
- Use `scripts/release.py` for releases. It auto-increments patch, validates `CHANGELOG.md` has notes for the new version, runs `just verify`, commits, pushes `dev`, fast-forwards `main`, tags, and pushes the tag.
- `cargo-dist` builds release artifacts via `.github/workflows/release.yml` for macOS (aarch64, x86_64), Linux (x86_64), and Windows (x86_64). Homebrew formulas are published to the `maximgorbatyuk/homebrew-tap` repo.

## Documentation maintenance

When you change behaviour, update the relevant doc in the same change:

- New/changed command, dependency, env var, install step, port, or onboarding step → update `README.md`.
- New/changed module/crate, runtime, dependency category, deployment target, or execution flow → update `docs/architecture.md`.
- New domain area or renamed domain directory → update `docs/domain.md`.
- New/changed event, runtime call, webhook, scheduled integration, external service, MCP tool, or filesystem write target → update `docs/interactions.md`.
- New test framework, test command, fixture pattern, or test-isolation requirement → update `docs/testing.md`.
- New non-obvious constraint, ordering rule, retry/idempotency rule, environment quirk, or surprising domain rule → update `docs/gotchas.md`.

If you remove a behaviour described in any doc, delete the relevant text in the same change. Do not let docs accumulate stale references.
