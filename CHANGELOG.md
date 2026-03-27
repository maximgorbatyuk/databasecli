# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
