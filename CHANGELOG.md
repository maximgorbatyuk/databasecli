# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
