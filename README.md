# databasecli

A CLI and TUI tool for managing and exploring PostgreSQL databases. Includes an MCP server for AI agent access.

## Features

- Full-screen TUI with interactive database management, health monitoring, schema browsing, and query execution
- CLI subcommands for scripting: `list`, `health-check`, `schema`, `query`, `analyze`, `summary`, `erd`, `compare`, `trend`, `sample`
- MCP server exposing 14 read-only tools for AI agents (Claude Desktop, Claude Code, and other MCP clients)
- Multi-database support — connect to specific databases with `--db` or all at once with `--all`
- INI-based configuration with per-database connection settings
- Cross-platform: macOS, Linux, Windows

## Installation

```bash
brew tap maximgorbatyuk/tap
brew install databasecli

# Check installation
databasecli --version
```

## Quick Start

Create a config file at `~/.databasecli/databases.ini`:

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

Then run:

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
