# databasecli

Manage your databases with AI agents. A CLI, TUI, and MCP server providing secure read-only access to PostgreSQL databases.

## For Whom

- **Backend Developers** — manage connections to multiple databases across environments. Switch between dev, staging, and production without remembering connection strings.
- **Database Administrators** — monitor database health at a glance. Run quick connectivity checks across your entire fleet and spot issues before they become incidents.
- **DevOps Engineers** — integrate health checks into your workflow. Script database connectivity verification or use the TUI for interactive troubleshooting.

## Features

- Full-screen TUI with interactive database management, health monitoring, schema browsing, and query execution
- CLI subcommands for scripting: `list`, `health-check`, `schema`, `query`, `analyze`, `summary`, `erd`, `compare`, `trend`, `sample`
- MCP server exposing 14 read-only tools for AI agents (Claude Desktop, Claude Code, and other MCP clients)
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

1. Create a config file at `~/.databasecli/databases.ini` (or `<project>/.databasecli/databases.ini` when using `-D`):

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
```

## MCP Server

The `databasecli-mcp` binary is a read-only MCP server that gives AI agents secure access to your PostgreSQL databases over stdio. All connections enforce read-only mode at both the server and client level.

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
