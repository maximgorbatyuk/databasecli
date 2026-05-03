# databasecli

Manage your databases with AI agents. A CLI, TUI, and MCP server providing secure read-only access to PostgreSQL databases.

## For Whom

- **Backend Developers** — manage connections to multiple databases across environments. Switch between dev, staging, and production without remembering connection strings.
- **Database Administrators** — monitor database health at a glance. Run quick connectivity checks across your entire fleet and spot issues before they become incidents.
- **DevOps Engineers** — integrate health checks into your workflow. Script database connectivity verification or use the TUI for interactive troubleshooting.

## Features

- Full-screen TUI with interactive database management, health monitoring, schema browsing, and query execution
- CLI subcommands for scripting: `list`, `health-check`, `schema`, `query`, `exec`, `analyze`, `summary`, `erd`, `compare`, `trend`, `sample`
- MCP server exposing 14 read-only tools for AI agents (Claude Desktop, Claude Code, and other MCP clients)
- Local SQL execution with `exec` for operators (single statement, one database, no MCP exposure)
- Destructive statement confirmation for `UPDATE`, `DELETE`, `DROP`, `TRUNCATE`, and `ALTER` (use `--yes` to bypass)
- Multi-database support — connect to specific databases with `--db` or all at once with `--all`
- INI-based configuration with per-database connection settings
- Cross-platform: macOS, Linux, Windows

## Installation

### macOS (Homebrew)

```bash
brew tap maximgorbatyuk/tap
brew install databasecli
brew install databasecli-mcp   # MCP server for AI agents

databasecli --version
```

### Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/maximgorbatyuk/databasecli/releases/latest/download/databasecli-installer.sh | sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/maximgorbatyuk/databasecli/releases/latest/download/databasecli-mcp-installer.sh | sh
```

### Windows

PowerShell:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/maximgorbatyuk/databasecli/releases/latest/download/databasecli-installer.ps1 | iex"
powershell -ExecutionPolicy ByPass -c "irm https://github.com/maximgorbatyuk/databasecli/releases/latest/download/databasecli-mcp-installer.ps1 | iex"
```

MSI installers are also available on the [releases page](https://github.com/maximgorbatyuk/databasecli/releases).

### From source

```bash
cargo install --git https://github.com/maximgorbatyuk/databasecli databasecli
cargo install --git https://github.com/maximgorbatyuk/databasecli databasecli-mcp
```

## Quick Start

### Option 1: Call `init` command

```bash
databasecli init
```

The command will create .ini file in directory `<project_path>/.databasecli/databases.ini`. Also, this command will write MCP server to the folder.

### Option 2: Manual setup

1. Create a config file at `.databasecli/databases.ini` in your project directory:

```ini
[production]
host = localhost
port = 5432
user = admin
password = secret123
dbname = myapp

[staging]
host = staging-db.example.com
port = 5432
user = readonly
password = secret456
dbname = myapp_staging
```

2. Create `.mcp.json` in your project root to enable AI agent access:

```json
{
  "mcpServers": {
    "databasecli": {
      "command": "databasecli-mcp",
      "args": ["-D", "."]
    }
  }
}
```

3. Run:

```bash
databasecli                              # launch TUI
databasecli list                         # list stored connections
databasecli health-check --all           # check all databases
databasecli schema --db production       # inspect schema
databasecli query --db production "SELECT count(*) FROM users"

# Local-only write/DDL execution. Asks `[y/N]` before destructive statements;
# pass --yes to bypass. Single statement only; not exposed via MCP.
databasecli exec --db staging "INSERT INTO feature_flags (name) VALUES ('beta')"
databasecli exec --db staging --yes "DELETE FROM sessions WHERE expired_at < now()"
```

### Choose `query` vs `exec`

| Need | Use | Notes |
| --- | --- | --- |
| Read data (`SELECT`, `SHOW`, `EXPLAIN`, `TABLE`) | `databasecli query` | Read-only path; supports multi-db workflows |
| Change schema/data (`INSERT`, `UPDATE`, `DELETE`, `ALTER`, `CREATE`, etc.) | `databasecli exec` | Local-only path on one database; destructive operations prompt unless `--yes` |

### Execution safety model

- `exec` is intentionally narrow in v1: one statement only, optional trailing semicolon, no `WITH`, no procedural bodies.
- `exec` opens a short-lived writable connection with `statement_timeout = '30s'`; it does not reuse read-only MCP/query sessions.
- `query` remains the default for all read-only exploration and analysis workflows.

## MCP Server

The `databasecli-mcp` binary is a read-only MCP server that gives AI agents secure access to your PostgreSQL databases over stdio. All connections enforce read-only mode at both the server and client level.

### MCP read-only guarantee (no change commands)

MCP clients cannot run state-changing SQL (`INSERT`, `UPDATE`, `DELETE`, `DROP`, `TRUNCATE`, `ALTER`, `CREATE`, `GRANT`, etc.). This is enforced at multiple layers:

1. **No write tool on MCP surface**: there is no MCP `exec`/`execute` tool.
2. **Read-only validator**: MCP `query`/`compare` reject write SQL and multi-statement smuggling attempts.
3. **Database-level protection**: MCP connections are created with `SET default_transaction_read_only = on`.

Write execution is reachable only through local operator paths (`databasecli exec` in CLI/TUI), and guard tests in `crates/databasecli-mcp/tests/guard.rs` ensure this boundary cannot regress silently.

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "databasecli": {
      "command": "databasecli-mcp",
      "args": ["-D", "/path/to/project"]
    }
  }
}
```

### Claude Code

Add to `.claude/settings.local.json` in your project (machine-specific, not committed):

```json
{
  "mcpServers": {
    "databasecli": {
      "command": "databasecli-mcp",
      "args": ["-D", "."]
    }
  }
}
```

Or to `.mcp.json` in your project root if you want to share the config with your team.

For the full tools reference and security model, see [`docs/mcp.md`](docs/mcp.md).

## License

MIT. See [LICENSE](LICENSE).
