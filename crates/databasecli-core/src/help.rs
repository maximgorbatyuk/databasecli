#[derive(Debug, Clone)]
pub struct HelpSection {
    pub title: String,
    pub items: Vec<HelpItem>,
}

#[derive(Debug, Clone)]
pub struct HelpItem {
    pub name: String,
    pub description: String,
}

pub fn build_help_sections() -> Vec<HelpSection> {
    vec![
        HelpSection {
            title: "CLI Commands".to_string(),
            items: vec![
                HelpItem {
                    name: "databasecli".to_string(),
                    description: "Launch interactive TUI (default)".to_string(),
                },
                HelpItem {
                    name: "databasecli tui".to_string(),
                    description: "Launch interactive TUI (explicit)".to_string(),
                },
                HelpItem {
                    name: "databasecli init [-D <path>]".to_string(),
                    description: "Create databases.ini and .mcp.json (idempotent)".to_string(),
                },
                HelpItem {
                    name: "databasecli list".to_string(),
                    description: "List all stored database connections from config".to_string(),
                },
                HelpItem {
                    name: "databasecli health".to_string(),
                    description: "Check connectivity for all configured databases".to_string(),
                },
                HelpItem {
                    name: "databasecli list-databases".to_string(),
                    description: "List currently connected databases".to_string(),
                },
                HelpItem {
                    name: "databasecli health-check".to_string(),
                    description: "Enhanced health: PG version, size, uptime".to_string(),
                },
                HelpItem {
                    name: "databasecli schema [--schema <name>]".to_string(),
                    description: "Dump schema: tables, columns, types, PKs, row counts, sizes"
                        .to_string(),
                },
                HelpItem {
                    name: "databasecli query <sql>".to_string(),
                    description: "Run a read-only SQL query (SELECT, WITH, EXPLAIN, SHOW)"
                        .to_string(),
                },
                HelpItem {
                    name: "databasecli exec --db <name> [--yes] <sql>".to_string(),
                    description:
                        "Local-only write/DDL on one database. Inline form runs a single statement (or a WITH ... INSERT|UPDATE|DELETE chain). Prompts before destructive statements; --yes bypasses. Not exposed via MCP."
                            .to_string(),
                },
                HelpItem {
                    name: "databasecli exec --db <name> --file <PATH> [--transaction] [--yes]"
                        .to_string(),
                    description:
                        "Run a multi-statement SQL script (seeds, migrations) on one database. Supports BEGIN/COMMIT/SAVEPOINT/SET and WITH ... DML chains. --transaction wraps the whole file in an injected BEGIN/COMMIT pair if your script doesn't manage transactions itself. Destructive statements are listed once with line numbers before running. Not exposed via MCP."
                            .to_string(),
                },
                HelpItem {
                    name: "databasecli sample <table> [--limit N] [--order-by <col>] [--schema <name>]".to_string(),
                    description: "Preview rows from a table (default: 20 rows)".to_string(),
                },
                HelpItem {
                    name: "databasecli analyze <table> [--schema <name>]".to_string(),
                    description: "Profile a table: nulls, cardinality, distributions, top values"
                        .to_string(),
                },
                HelpItem {
                    name: "databasecli summary".to_string(),
                    description: "Database overview: table counts, sizes, largest tables"
                        .to_string(),
                },
                HelpItem {
                    name: "databasecli erd [--schema <name>] [--format ascii|mermaid|dot] [--output <file>]".to_string(),
                    description: "Entity-relationship diagram: PKs and foreign keys".to_string(),
                },
                HelpItem {
                    name: "databasecli compare <sql>".to_string(),
                    description: "Run same query across all connected databases, compare results"
                        .to_string(),
                },
                HelpItem {
                    name: "databasecli trend <table> --timestamp <col> [--interval day|week|month|year] [--value <col>] [--schema <name>] [--limit N]".to_string(),
                    description: "Time-series: counts/averages grouped by interval".to_string(),
                },
                HelpItem {
                    name: "databasecli reference".to_string(),
                    description: "Show this full help reference".to_string(),
                },
                HelpItem {
                    name: "databasecli help | -h | --help".to_string(),
                    description: "Show clap-generated command list".to_string(),
                },
            ],
        },
        HelpSection {
            title: "Global Flags".to_string(),
            items: vec![
                HelpItem {
                    name: "-D, --directory <path>".to_string(),
                    description: "Working directory for config resolution".to_string(),
                },
                HelpItem {
                    name: "--db <name>".to_string(),
                    description: "Database to connect to (repeatable)".to_string(),
                },
                HelpItem {
                    name: "--all".to_string(),
                    description: "Connect to all configured databases".to_string(),
                },
                HelpItem {
                    name: "-h, --help".to_string(),
                    description: "Show clap-generated help for any command".to_string(),
                },
                HelpItem {
                    name: "-V, --version".to_string(),
                    description: "Show version".to_string(),
                },
            ],
        },
        HelpSection {
            title: "TUI Key Bindings".to_string(),
            items: vec![
                HelpItem {
                    name: "Up / k".to_string(),
                    description: "Navigate up in menu, scroll up in results".to_string(),
                },
                HelpItem {
                    name: "Down / j".to_string(),
                    description: "Navigate down in menu, scroll down in results".to_string(),
                },
                HelpItem {
                    name: "Enter".to_string(),
                    description: "Select menu item, submit input, enter typing mode".to_string(),
                },
                HelpItem {
                    name: "Esc".to_string(),
                    description: "Go back to home, stop typing".to_string(),
                },
                HelpItem {
                    name: "i".to_string(),
                    description: "Enter typing mode (Query, Sample, Analyze, Compare, Trend)"
                        .to_string(),
                },
                HelpItem {
                    name: "Space".to_string(),
                    description: "Toggle database selection (Connect screen)".to_string(),
                },
                HelpItem {
                    name: "q".to_string(),
                    description: "Quit the application".to_string(),
                },
            ],
        },
        HelpSection {
            title: "Configuration".to_string(),
            items: vec![
                HelpItem {
                    name: "Config file".to_string(),
                    description: "<cwd>/.databasecli/databases.ini".to_string(),
                },
                HelpItem {
                    name: "With -D flag".to_string(),
                    description: "<directory>/.databasecli/databases.ini".to_string(),
                },
                HelpItem {
                    name: "DATABASECLI_CONFIG_PATH".to_string(),
                    description: "Env var override — uses this exact path if set (highest priority)".to_string(),
                },
                HelpItem {
                    name: "INI format".to_string(),
                    description: "[section_name] with host, port, user, password, dbname fields".to_string(),
                },
                HelpItem {
                    name: "[settings] section".to_string(),
                    description: "query_limit = 500 (max rows returned from user queries, 0 = unlimited)".to_string(),
                },
            ],
        },
        HelpSection {
            title: "MCP Server (AI Agent Integration)".to_string(),
            items: vec![
                HelpItem {
                    name: "Build".to_string(),
                    description: "cargo build -p databasecli-mcp --release".to_string(),
                },
                HelpItem {
                    name: "Install".to_string(),
                    description: "cargo install --path crates/databasecli-mcp".to_string(),
                },
                HelpItem {
                    name: "Init config".to_string(),
                    description: "databasecli init [-D <path>] (creates databases.ini + .mcp.json)".to_string(),
                },
                HelpItem {
                    name: "Run".to_string(),
                    description: "databasecli-mcp [-D <path>] (communicates via stdio JSON-RPC)".to_string(),
                },
                HelpItem {
                    name: "Claude Desktop".to_string(),
                    description: "Add to claude_desktop_config.json mcpServers section".to_string(),
                },
                HelpItem {
                    name: "Claude Code".to_string(),
                    description: "Add to .mcp.json in project root".to_string(),
                },
                HelpItem {
                    name: "14 tools".to_string(),
                    description: "list_configured, connect, disconnect, list_connected, query, schema, sample, analyze, compare, summary, erd, trend, enhanced_health, suggest_migration".to_string(),
                },
                HelpItem {
                    name: "Writes via MCP".to_string(),
                    description: "Never. INSERT/UPDATE/DELETE/DROP/TRUNCATE/ALTER/CREATE/GRANT are unavailable to MCP clients by design — only `databasecli exec` (local CLI/TUI) can run them.".to_string(),
                },
            ],
        },
        HelpSection {
            title: "Security".to_string(),
            items: vec![
                HelpItem {
                    name: "Read-only enforcement".to_string(),
                    description: "SET default_transaction_read_only = on (server-side) + client-side SQL validation".to_string(),
                },
                HelpItem {
                    name: "Statement timeout".to_string(),
                    description: "SET statement_timeout = '30s' on every connection (read-only and writable alike)".to_string(),
                },
                HelpItem {
                    name: "SQL validation (query)".to_string(),
                    description: "Only SELECT, WITH, EXPLAIN, SHOW, TABLE allowed. Semicolons blocked.".to_string(),
                },
                HelpItem {
                    name: "exec scope (inline)".to_string(),
                    description: "One statement per call. WITH ... <INSERT|UPDATE|DELETE> chains accepted; classified by the most severe verb across all CTE bodies and the outer DML. Procedural bodies (DO $$ ... $$, function definitions) and dollar-quoted strings are rejected. COPY rejected. Destructive verbs (UPDATE/DELETE/DROP/TRUNCATE/ALTER/MERGE) prompt unless --yes.".to_string(),
                },
                HelpItem {
                    name: "exec scope (--file)".to_string(),
                    description: "Multi-statement scripts split by unquoted `;`. BEGIN/COMMIT/ROLLBACK/SAVEPOINT/SET accepted as their own statements. Destructive scan is global: one prompt lists every destructive statement with line numbers. --transaction wraps the whole script in an injected BEGIN/COMMIT pair when the file doesn't manage transactions itself. Procedural bodies still rejected.".to_string(),
                },
                HelpItem {
                    name: "exec connection".to_string(),
                    description: "Short-lived writable connection per invocation. Inline runs use one connection for one statement; --file uses one connection for the entire script so BEGIN/COMMIT/SAVEPOINT work as written. Never held in the read-only ConnectionManager; never reachable from MCP.".to_string(),
                },
                HelpItem {
                    name: "Passwords".to_string(),
                    description: "Never exposed to MCP agents. Stored in INI file (chmod 600 recommended).".to_string(),
                },
            ],
        },
    ]
}

pub fn format_help_text(sections: &[HelpSection]) -> String {
    let mut out = String::new();
    out.push_str("databasecli — PostgreSQL database connection manager\n\n");

    for section in sections {
        out.push_str(&format!("{}:\n", section.title));

        let name_width = section
            .items
            .iter()
            .map(|i| i.name.len())
            .max()
            .unwrap_or(20)
            .min(50);

        for item in &section.items {
            if item.name.len() > 50 {
                out.push_str(&format!("  {}\n", item.name));
                out.push_str(&format!(
                    "  {:>width$}{}\n",
                    "",
                    item.description,
                    width = 4
                ));
            } else {
                out.push_str(&format!(
                    "  {:<width$}  {}\n",
                    item.name,
                    item.description,
                    width = name_width
                ));
            }
        }
        out.push('\n');
    }

    out
}
