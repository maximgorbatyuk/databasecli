# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

databasecli is a Rust CLI/TUI tool for managing PostgreSQL database connections. It ships a single binary (`databasecli`) that defaults to a full-screen TUI and also exposes `list` and `health` subcommands for scripting.

## Architecture

Rust workspace with one binary crate and two library crates:

- **databasecli-cli** — Binary entrypoint, `clap` command parsing, launches TUI or runs subcommands
- **databasecli-core** — Shared domain types (`DatabaseConfig`, `HealthResult`), INI config parsing (`configparser`), PostgreSQL health checking (`postgres` crate, synchronous), error types
- **databasecli-tui** — TUI app state, event loop, rendering (thin presentation layer, no domain logic)

Domain logic lives in library crates so both TUI and CLI call the same services.

## Build Commands

```bash
cargo run                                # run databasecli (dev build, launches TUI)
cargo run -- list                        # list stored databases
cargo run -- health                      # check database connectivity
cargo build --workspace                  # build all crates
cargo build --workspace --release        # release build
cargo test --workspace                   # run all tests
cargo test -p databasecli-core           # test a single crate
cargo test -p databasecli-core config    # run tests matching "config" in one crate
cargo fmt --all --check                  # format check
cargo clippy --workspace --all-targets --all-features -- -D warnings  # lint
```

A `justfile` exists with the same targets (`just verify`, `just test`, etc.).

## CLI Usage

```bash
databasecli                              # launch TUI (default)
databasecli tui                          # launch TUI (explicit)
databasecli list                         # list all stored database connections
databasecli health                       # check health of all databases
```

## Configuration

INI file at `<cwd>/.databasecli/databases.ini` (resolved from the current working directory):

```ini
[production]
host = localhost
port = 5432
user = admin
password = secret123
dbname = myapp
```

Env var `DATABASECLI_CONFIG_PATH` overrides the config path (useful for testing).

Config is project-local: resolved relative to the current working directory. Path resolution no longer depends on `cfg!(debug_assertions)` or the `home` crate for the default path.

## Design Constraints

- Synchronous PostgreSQL connections (no tokio/async) — uses `postgres` crate directly
- TUI is a thin presentation layer — screens never perform database operations directly
- TUI health checks run on a background thread via `mpsc::channel`; the event loop uses `poll(100ms)` for non-blocking input
- Missing config file returns an empty list with a helpful message (no crash)
- Connection timeout set to 5 seconds to keep health checks responsive
- Passwords stored in plain text in INI file (v1 limitation)

## Testing

- Config parsing tests use `tempfile::NamedTempFile` for isolated INI files
- Health check tests require a live database and are not included in the default test suite
- Tests use `DATABASECLI_CONFIG_PATH` env var for path isolation

## Conventions

- Commit using conventional commits (`feat:`, `fix:`, `chore:`, `test:`, `docs:`)
- CI runs on push to `main`/`dev` and PRs to `main`: fmt check, clippy, build, test
- CI runs Windows build + test alongside Linux checks
- Release via `cargo-dist` with GitHub Actions; macOS, Linux, and Windows

## Rust 2024 Edition Gotchas

- Use `std::io::Error::other()` not `Error::new(ErrorKind::Other, ...)`
- crossterm on Windows fires both `KeyEventKind::Press` and `KeyEventKind::Release` — always filter `key.kind == KeyEventKind::Press` in the event loop
