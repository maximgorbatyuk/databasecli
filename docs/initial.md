# Plan: Build `databasecli`

## Context

The user wants a new Rust CLI/TUI app for managing PostgreSQL database connections, following the same architecture and tooling patterns as the sibling `repolyze` project. The directory `/Users/maximgorbatyuk/projects/cli/databasecli/` exists with `.git`, `.gitignore`, `LICENSE`, `README.md` only.

## Architecture

Rust workspace with 3 crates under `crates/`:

```
databasecli/
├── Cargo.toml                              (workspace root)
├── dist-workspace.toml                     (cargo-dist)
├── justfile
├── .editorconfig
├── CLAUDE.md
├── .github/workflows/ci.yml
├── crates/
│   ├── databasecli-cli/                    (binary: databasecli)
│   │   └── src/ {main.rs, args.rs, run.rs}
│   ├── databasecli-core/                   (domain: config parsing, health checks)
│   │   └── src/ {lib.rs, config.rs, health.rs, error.rs}
│   └── databasecli-tui/                    (TUI: ratatui + crossterm)
│       └── src/ {lib.rs, app.rs, ui.rs, event.rs}
```

- **databasecli-cli** — entrypoint, clap parsing, dispatches to TUI or CLI handlers
- **databasecli-core** — INI config parsing (`configparser` crate), PostgreSQL health checks (`postgres` crate, synchronous), shared types
- **databasecli-tui** — interactive TUI (ratatui 0.29 + crossterm 0.28), same event loop pattern as repolyze

## CLI Design

```
databasecli              → launches TUI (default, same as repolyze)
databasecli tui          → launches TUI (explicit)
databasecli list         → prints stored databases to stdout
databasecli health       → checks all databases, prints health table
```

## Config File

`~/.databasecli/databases.ini` (release) or `target/debug/databases-dev.ini` (dev):

```ini
[production]
host = localhost
port = 5432
user = admin
password = secret123
dbname = myapp
```

Env var `DATABASECLI_CONFIG_PATH` overrides for testing. Missing file → empty list with helpful message (no crash).

## TUI Screens

- **Home** — logo + menu: "Stored Databases", "Database Health"
- **StoredDatabases** — lists all connections (name, host:port, dbname, user)
- **DatabaseHealth** — color-coded status (green=Connected, red=Failed) with response times; health check runs on background thread via `mpsc::channel`

Key bindings follow repolyze: `q`=quit, `↑/k`/`↓/j`=navigate, `Enter`=select, `Esc`=back.

## Implementation Phases

### Phase 1: Scaffolding
Create workspace `Cargo.toml`, all three crate `Cargo.toml` files, `.editorconfig`, `justfile`, `dist-workspace.toml`. Update `.gitignore`. Stub `lib.rs`/`main.rs` files so `cargo check --workspace` passes.

**Files:** `Cargo.toml`, `crates/databasecli-cli/Cargo.toml`, `crates/databasecli-core/Cargo.toml`, `crates/databasecli-tui/Cargo.toml`, `.editorconfig`, `justfile`, `dist-workspace.toml`, stub sources

### Phase 2: databasecli-core (domain logic)
Implement `error.rs` (DatabaseCliError enum), `config.rs` (DatabaseConfig, ConnectionStore, INI parsing via `configparser`, path resolution with `home` crate), `health.rs` (HealthResult, HealthStatus, check_health using `postgres` crate with 5s connect_timeout, format_health_table for CLI output). Unit tests for config parsing.

**Files:** `crates/databasecli-core/src/{lib.rs, error.rs, config.rs, health.rs}`

### Phase 3: databasecli-cli (binary, CLI commands)
Implement `args.rs` (Cli struct, Commands enum with Tui/List/Health), `run.rs` (run_list, run_health), `main.rs` (parse → dispatch). TUI dispatch calls `databasecli_tui::run()`.

**Files:** `crates/databasecli-cli/src/{main.rs, args.rs, run.rs}`

### Phase 4: databasecli-tui (interactive UI)
Implement `app.rs` (Screen enum, MenuItem enum, AppState, AppAction), `event.rs` (handle_key dispatched by screen), `ui.rs` (draw functions: home menu with `"➤ "` selected prefix, databases list, health status with colors), `lib.rs` (event loop: raw mode, 100ms poll, background health thread via mpsc, spinner animation).

**Files:** `crates/databasecli-tui/src/{lib.rs, app.rs, event.rs, ui.rs}`

### Phase 5: CI/CD and distribution
Create `.github/workflows/ci.yml` (format, clippy, build, test on ubuntu + windows). Write `CLAUDE.md`.

**Files:** `.github/workflows/ci.yml`, `CLAUDE.md`

## Key Dependencies

| Crate | Key deps |
|-------|----------|
| databasecli-core | `configparser 3`, `home 0.5`, `postgres 0.19` (with rustls or native-tls), `thiserror 2` |
| databasecli-tui | `crossterm 0.28`, `ratatui 0.29`, `databasecli-core` (path) |
| databasecli-cli | `clap 4` (workspace), `anyhow 1` (workspace), `databasecli-core`, `databasecli-tui` (path) |

## Design Decisions

- **Synchronous postgres** (no tokio) — matches repolyze's no-async approach
- **Background thread for health checks** — avoids blocking TUI event loop on slow/unreachable DBs
- **configparser for INI** — lightweight, no async, well-maintained
- **5-second connect_timeout** — keeps health checks responsive
- **`cfg!(debug_assertions)` for path resolution** — dev uses `target/debug/`, release uses `~/.databasecli/`
- **Plain-text passwords in INI** — noted as v1 limitation, can be improved later

## Verification

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --workspace --all-targets
cargo test --workspace
cargo run -- list       # prints stored databases
cargo run -- health     # checks database connectivity
cargo run               # launches TUI
```
