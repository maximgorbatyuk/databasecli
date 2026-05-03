//! MCP read-only guarantee guards.
//!
//! These tests fail if the MCP surface ever gains a write/execute tool,
//! or if the read-only validator stops rejecting writes. Both must hold
//! for the project's documented security model.

use databasecli_core::commands::query::validate_readonly;

const SERVER_RS: &str = include_str!("../src/server.rs");
const TOOLS_QUERY_RS: &str = include_str!("../src/tools/query.rs");
const TOOLS_CONNECTION_RS: &str = include_str!("../src/tools/connection.rs");
const TOOLS_ANALYSIS_RS: &str = include_str!("../src/tools/analysis.rs");
const TOOLS_MIGRATION_RS: &str = include_str!("../src/tools/migration.rs");
const TOOLS_SCHEMA_RS: &str = include_str!("../src/tools/schema.rs");

const ALL_MCP_SOURCES: &[(&str, &str)] = &[
    ("src/server.rs", SERVER_RS),
    ("src/tools/query.rs", TOOLS_QUERY_RS),
    ("src/tools/connection.rs", TOOLS_CONNECTION_RS),
    ("src/tools/analysis.rs", TOOLS_ANALYSIS_RS),
    ("src/tools/migration.rs", TOOLS_MIGRATION_RS),
    ("src/tools/schema.rs", TOOLS_SCHEMA_RS),
];

/// Primary guard: the MCP crate must never reach the writable execution path.
///
/// `execute_statement` is the only function in `databasecli-core` that runs
/// arbitrary SQL against a writable connection. If it ever appears in the MCP
/// crate, AI agents can write through MCP — which the project documents as
/// impossible.
#[test]
fn mcp_does_not_reference_execute_statement() {
    for (path, src) in ALL_MCP_SOURCES {
        assert!(
            !src.contains("execute_statement"),
            "{path} references `execute_statement`. The MCP surface must stay read-only; see docs/plans/execution.md."
        );
    }
}

/// Primary guard: the MCP crate must never expose a tool whose name suggests
/// write execution. The rmcp `#[tool(...)]` macro registers each `fn`-name
/// (snake_case) as the tool name, so every `fn` declared in `server.rs` either
/// is a tool or supports the server lifecycle. Either way it must appear in
/// this allowlist — anything new forces a deliberate decision.
///
/// To add a new read-only tool: add its fn name here.
/// To add a write tool: don't. Build it as a CLI subcommand instead.
#[test]
fn mcp_server_fns_are_allowlisted() {
    const ALLOWED: &[&str] = &[
        // Server lifecycle
        "new",
        "get_info",
        // Read-only MCP tools
        "list_configured_databases",
        "connect_databases",
        "disconnect_databases",
        "list_connected_databases",
        "query",
        "schema",
        "sample",
        "compare",
        "analyze",
        "summary",
        "erd",
        "trend",
        "enhanced_health",
        "suggest_migration",
    ];

    for name in fn_names_in(SERVER_RS) {
        assert!(
            ALLOWED.contains(&name.as_str()),
            "server.rs declares unexpected fn `{name}`. If this is a new tool, \
             confirm it is read-only and add it to the ALLOWED list in \
             crates/databasecli-mcp/tests/guard.rs. Write/exec tools are not allowed."
        );
    }
}

/// Extract every top-level `fn NAME(...)` identifier from a Rust source file.
/// Tolerates `pub`, `pub(crate)`, and `async` prefixes. Skips anything that
/// looks like a use-as alias or a string occurrence by requiring `(` after
/// the identifier.
fn fn_names_in(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim_start();
        // Strip up to two prefix keywords (pub / async / pub(...))
        let after_prefix = strip_prefix_keywords(trimmed);
        let Some(rest) = after_prefix.strip_prefix("fn ") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        // Must be a real fn declaration: identifier immediately followed by
        // `(` or `<` (for generics). Excludes `fn` appearing inside comments
        // or doc strings that don't continue with a paren.
        let after_name = rest[name.len()..].trim_start();
        if after_name.starts_with('(') || after_name.starts_with('<') {
            out.push(name);
        }
    }
    out
}

fn strip_prefix_keywords(s: &str) -> &str {
    let mut cur = s;
    for _ in 0..3 {
        cur = cur.trim_start();
        if let Some(rest) = cur.strip_prefix("pub(crate)") {
            cur = rest;
        } else if let Some(rest) = cur.strip_prefix("pub") {
            cur = rest;
        } else if let Some(rest) = cur.strip_prefix("async") {
            cur = rest;
        } else if let Some(rest) = cur.strip_prefix("const") {
            cur = rest;
        } else {
            break;
        }
    }
    cur.trim_start()
}

#[test]
fn fn_names_in_extracts_basic_signatures() {
    let src = r#"
        fn alpha() {}
        pub fn beta(x: i32) {}
        pub async fn gamma(&self) -> Result<()> { Ok(()) }
        // fn not_a_real_fn
        fn delta<T>(t: T) {}
    "#;
    let names = fn_names_in(src);
    assert_eq!(names, vec!["alpha", "beta", "gamma", "delta"]);
}

/// Behavioural guard: the read-only validator that backs MCP `query` must
/// reject every write classification. If this ever passes, MCP `query` would
/// silently start running writes.
#[test]
fn mcp_query_validator_rejects_writes() {
    for sql in [
        "INSERT INTO t VALUES (1)",
        "UPDATE t SET x = 1",
        "DELETE FROM t",
        "DROP TABLE t",
        "TRUNCATE t",
        "ALTER TABLE t ADD COLUMN y int",
        "CREATE TABLE t (id int)",
        "GRANT ALL ON t TO public",
    ] {
        assert!(
            validate_readonly(sql).is_err(),
            "validate_readonly accepted write SQL: {sql}"
        );
    }
}

/// Behavioural guard: the read-only validator must reject multi-statement
/// payloads that could smuggle a write past the leading-keyword check.
#[test]
fn mcp_query_validator_rejects_multi_statement_smuggling() {
    assert!(validate_readonly("SELECT 1; DROP TABLE t").is_err());
    assert!(validate_readonly("SELECT 1;DELETE FROM t").is_err());
}
