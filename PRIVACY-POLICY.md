# Privacy Policy

Last updated: 2026-03-27

This Privacy Policy explains how `databasecli` handles information when you use the open source project, its source code repository, and official release artifacts.

`databasecli` is designed as a local database connection management tool. It stores PostgreSQL connection configurations on your machine and performs health checks and queries against databases you configure.

## Scope

This policy applies to:

- the official `databasecli` source repository;
- official binaries and release artifacts published by the project maintainer; and
- documentation distributed with the project.

This policy does not automatically apply to forks, third-party builds, package mirrors, or modified versions of the software. Those distributions may follow different privacy practices.

## Information `databasecli` may process locally

When you run `databasecli`, the software may access and process information from the databases and configuration files you provide, including:

- database connection parameters (host, port, username, database name);
- database passwords stored in your INI configuration file;
- database health check results (connectivity status, response times);
- database schema metadata (table names, column names, row counts, sizes) when using the MCP server;
- query results returned by read-only SQL queries you or an AI agent execute through the MCP server.

By design, this processing is performed locally on your device against databases you explicitly configure.

## What the project does not do by default

Based on the published project documentation and repository contents at the time of this policy:

- `databasecli` does not require a user account;
- `databasecli` does not operate a hosted service;
- `databasecli` does not send connection details, query results, or health check data to the project maintainer by default;
- `databasecli` does not include built-in advertising or behavioral tracking;
- `databasecli` does not connect to any database unless you explicitly configure it.

If the project adds optional networked features in a future release, this policy should be updated before or when those features are released.

## How information is used

Information processed by `databasecli` is used to:

- store and manage PostgreSQL connection configurations selected by the user;
- perform health checks against configured databases;
- display connection status and database metadata in the terminal user interface;
- execute read-only SQL queries through the MCP server when used with AI agents; and
- return schema, sample data, and analysis results requested by the user or AI agent.

## Where processing happens

The intended privacy model for `databasecli` is local processing on the user's machine.

That means database connections, health checks, and queries are initiated from your device. Configuration files are stored locally in the location you choose (default: `~/.databasecli/databases.ini`).

## MCP server considerations

The `databasecli-mcp` server enables AI agents to interact with your databases via the Model Context Protocol. When using this feature:

- all database connections are read-only (enforced at both server and PostgreSQL levels);
- database passwords are never exposed to the AI agent;
- databases are referenced by name only;
- a 30-second statement timeout prevents runaway queries;
- SQL is validated to allow only SELECT, WITH, EXPLAIN, SHOW, and TABLE statements.

The AI agent may see database schema metadata, table contents (via read-only queries), and health check results. Review your AI agent's privacy policy for how it handles data received through MCP.

## Sharing and disclosure

The project maintainer does not receive your database data from the software by default.

Information may still be disclosed by you or your environment if you choose to:

- share health check output or query results;
- paste command output into issue trackers, chats, or AI tools;
- use the MCP server with AI agents that transmit data to external services;
- run the software on systems monitored by your employer, hosting provider, or device management tools.

Please review generated output before sharing it. Database query results may reveal table structures, data contents, connection details, or other sensitive information.

## Command-line and local environment considerations

Like many CLI tools, use of `databasecli` may expose limited operational data to your local environment, including:

- shell history containing commands you ran;
- terminal scrollback or log capture tools;
- operating system file access records;
- configuration files containing database passwords in plaintext.

These behaviors are generally controlled by your shell, operating system, and development environment rather than by the project maintainer. Ensure your configuration file has restricted permissions (`chmod 600`).

## Third-party services and distribution channels

The project may be distributed or hosted through third-party services, including:

- GitHub, for source hosting and release distribution;
- GitHub Actions, for release automation; and
- Homebrew, for package installation on macOS.

If you download the software, browse the repository, install via Homebrew, or interact with GitHub-hosted project resources, those third parties may collect information under their own privacy policies and terms. The `databasecli` project does not control those third-party practices.

## Website analytics

The project website at `https://databasecli.app` uses Google Analytics to collect anonymous usage statistics (page views, referrers, device type). No personal data is collected through the website beyond what Google Analytics processes under its own privacy policy.

## Data retention

Because `databasecli` is designed for local use, the project maintainer does not retain your database data by default through the software itself.

Retention of any generated data depends on your environment and choices, such as whether you:

- keep configuration files containing database credentials;
- preserve terminal logs or shell history;
- share query results or health check output with external services; or
- use third-party package managers or hosting platforms.

You can generally delete local configuration and related artifacts directly from your system when you no longer need them.

## Security

`databasecli` is intended to minimize privacy risk by operating locally without requiring a hosted backend.

However, no software environment is completely risk-free. You are responsible for:

- choosing which databases to configure and connect to;
- protecting access to your machine and configuration files;
- securing database credentials stored in the INI configuration file;
- reviewing query results before sharing them externally; and
- evaluating AI agent data handling when using the MCP server.

If you use `databasecli` with databases that contain confidential, proprietary, personal, or regulated information, evaluate that use under your own legal, contractual, and security obligations.

## Open source development and forks

Because `databasecli` is open source, anyone may inspect the source code and, subject to the license, create modified versions. Modified versions, forks, unofficial packages, or downstream distributions may behave differently from the official project and may introduce new data practices.

You should review the source, release notes, and privacy terms for any non-official distribution you use.

## Children's privacy

`databasecli` is a developer tool and is not directed to children.

## Changes to this policy

This Privacy Policy may be updated as the project evolves. Material changes should be reflected in the repository so users can review the current version before using new features.

## Contact

For questions about this Privacy Policy or the official project, please use the project's public repository:

- `https://github.com/maximgorbatyuk/databasecli`

If you need to report a privacy or security concern, please open an appropriate issue or contact channel provided through the official repository.
