# Testing

For service identity, the detected stack, and high-level commands, see [`../README.md`](../README.md). For the workspace layout and which crate owns what, see [`./architecture.md`](./architecture.md).

## Frameworks

- All tests use Rust's built-in test harness (`cargo test`). No alternative runner.
- Test fixtures use [`tempfile`](../crates/databasecli-core/Cargo.toml) for temporary INI files and config directories.
- There is no end-to-end test suite. There is no headless browser, emulator, or container required.
- There are no tests that hit a live PostgreSQL server in the default suite. The `health` and `health-check` paths require a live database to exercise; the unit tests stop at validators, formatters, and config logic.

## Commands

| Purpose | Command | Source |
|---------|---------|--------|
| Run the full suite (all crates) | `cargo test --workspace` | [`../justfile`](../justfile), [`../.github/workflows/ci.yml`](../.github/workflows/ci.yml) |
| Run one crate's tests | `cargo test -p <crate>` (e.g. `cargo test -p databasecli-core`) | per-crate `Cargo.toml` |
| Run tests matching a pattern | `cargo test -p <crate> <pattern>` (e.g. `cargo test -p databasecli-core config`) | Rust test harness convention |
| Format check | `cargo fmt --all --check` | [`../justfile`](../justfile) |
| Clippy with warnings as errors | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | [`../justfile`](../justfile), [`../.github/workflows/ci.yml`](../.github/workflows/ci.yml) |
| Aggregated verification | `just verify` (fmt-check, clippy, test, check) | [`../justfile`](../justfile) |

CI runs the same commands on Linux and the build/test pair on Windows — see [`../.github/workflows/ci.yml`](../.github/workflows/ci.yml).

## Layout

| Test location | Kind | Purpose |
|---------------|------|---------|
| `crates/databasecli-core/src/**/*.rs` (`#[cfg(test)] mod tests`) | Unit tests | Config parsing, init writers, read-only validator, exec validator/classifier, formatters |
| [`crates/databasecli-core/src/commands/execute/tests.rs`](../crates/databasecli-core/src/commands/execute/tests.rs) | Unit tests (split out) | Edge cases for `validate_single_statement` and `classify_statement` |
| `crates/databasecli-cli/src/run.rs` (`#[cfg(test)]`) | Unit tests | `clap` parsing for `exec` flag combinations (`--all`, multiple `--db`, `--yes`) |
| `crates/databasecli-tui/src/app.rs` (`#[cfg(test)]`) | Unit tests | `Execute` screen state machine: phase transitions, destructive confirmation, picker behaviour |
| [`crates/databasecli-mcp/tests/guard.rs`](../crates/databasecli-mcp/tests/guard.rs) | Integration tests | MCP read-only invariants (no `execute_statement` reference, fn-name allowlist, validator behavioural guards) |

When adding new tests, follow the existing co-location convention: domain logic tests live next to the code in `databasecli-core`; `clap` and TUI tests live next to those layers; cross-crate invariants belong in `crates/databasecli-mcp/tests/`.

## Fixture and isolation patterns

- **INI files.** Use `tempfile::NamedTempFile` (or `tempfile::tempdir()` for nested layouts like Codex's `.codex/config.toml`). Write the test content with `write_all`, then point the loader at the temp path. Examples: [`config.rs`](../crates/databasecli-core/src/config.rs) (`write_ini` helper), [`init.rs`](../crates/databasecli-core/src/init.rs).
- **Config-path isolation.** When tests need to exercise the env-var override, set `DATABASECLI_CONFIG_PATH` (or remove it deliberately) — see the `default_path_uses_cwd` test in [`config.rs`](../crates/databasecli-core/src/config.rs). The `unsafe { env::remove_var(...) }` block is a deliberate concession to Rust 2024's `env::set_var`/`remove_var` being marked unsafe; the test runs single-threaded.
- **TUI state machine.** Construct `AppState` directly via `AppState::new(...)`, then drive it through the same public methods the event loop uses (`update_connection_state`, `activate_selected`, `execute_picker_confirm`, `execute_submit_sql`, etc.). Tests assert on the public phase enum and on `take_action()` to confirm the right `AppAction` was emitted.
- **MCP read-only guards.** [`guard.rs`](../crates/databasecli-mcp/tests/guard.rs) reads each MCP source file as a string at compile time via `include_str!` and asserts (a) no source string contains `execute_statement`, (b) every top-level `fn` in `server.rs` is on the allowlist, (c) `validate_readonly` keeps rejecting every write classification and multi-statement smuggling. These tests must keep passing on every change.

## Test utilities

There are no fakes/mocks crates and no shared `tests/common/` modules. The codebase keeps the surface small enough that direct construction of value types (`DatabaseConfig`, `AppState`, `NormalizedStatement`) is preferred to test doubles.

## A typical test pattern

A representative pattern from the read-only validator: drive `validate_readonly` with concrete SQL strings and assert on the error path, including comment-stripping edge cases like `/* SELECT */ DELETE FROM users` (the comment must not hide the leading verb). The full set of assertions lives in [`crates/databasecli-core/src/commands/query.rs`](../crates/databasecli-core/src/commands/query.rs); use it as the reference for new validator coverage.
