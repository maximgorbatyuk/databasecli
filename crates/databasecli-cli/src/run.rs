use anyhow::Result;
use databasecli_core::commands::analyze::{analyze_table, format_table_profile};
use databasecli_core::commands::compare::{compare_query, format_compare_result};
use databasecli_core::commands::erd::{
    build_erd, format_erd_ascii, format_erd_dot, format_erd_mermaid,
};
use databasecli_core::commands::execute::{
    ScriptStatement, StatementKind, execute_normalized, execute_script, format_execute_result,
    format_script_results, split_script, validate_single_statement,
};
use databasecli_core::commands::export::{
    ExportFormat, ExportRequest, export, query_request, table_request,
};
use databasecli_core::commands::health::{check_all_enhanced_health, format_enhanced_health_table};
use databasecli_core::commands::list_databases::{format_connected_table, list_connected};
use databasecli_core::commands::query::{
    OutputFormat, execute_query, format_query_data, format_query_summary,
};
use databasecli_core::commands::sample::{format_sample, sample_table};
use databasecli_core::commands::schema::{dump_schema, format_schema};
use databasecli_core::commands::summary::{format_summary, summarize};
use databasecli_core::commands::trend::{TrendInterval, TrendParams, compute_trend, format_trend};
use databasecli_core::config::{
    Settings, load_databases, load_settings, normalize_statement_timeout,
    resolve_config_path_with_base,
};
use databasecli_core::connection::{ConnectionManager, connect_for_local_exec};
use databasecli_core::error::DatabaseCliError;
use databasecli_core::health::{check_all_health, format_health_table};

use databasecli_core::help::{build_help_sections, format_help_text};

use crate::args::Cli;

pub fn run_init(cli: &Cli) -> Result<()> {
    use std::io::{Write, stdin, stdout};

    use databasecli_core::init::{CodingAgent, FileAction};

    let agents = CodingAgent::ALL;
    let mut selected = vec![false; agents.len()];

    println!("Select coding agents to configure MCP for:");
    println!("(enter numbers separated by spaces, e.g. \"1 3\")\n");
    for (i, agent) in agents.iter().enumerate() {
        println!("  {}. {} ({})", i + 1, agent, agent.config_filename());
    }
    print!("\n> ");
    stdout().flush()?;

    let mut input = String::new();
    stdin().read_line(&mut input)?;

    for token in input.split_whitespace() {
        match token.parse::<usize>() {
            Ok(n) if n >= 1 && n <= agents.len() => {
                selected[n - 1] = true;
            }
            _ => {
                eprintln!("Ignoring unrecognized input: {token}");
            }
        }
    }

    let chosen: Vec<CodingAgent> = agents
        .iter()
        .enumerate()
        .filter(|(i, _)| selected[*i])
        .map(|(_, a)| *a)
        .collect();

    if chosen.is_empty() {
        println!("No agents selected. Skipping MCP configuration.");
        println!("Creating config file only.");
    }

    let result = databasecli_core::init::run_init(cli.directory.as_deref(), &chosen)?;

    match result.config_action {
        FileAction::Created => println!("Config created at {}", result.config_path.display()),
        FileAction::Unchanged => {
            println!("Config already exists at {}", result.config_path.display())
        }
        FileAction::Updated => unreachable!(),
    }

    for agent_result in &result.agent_results {
        match agent_result.action {
            FileAction::Created => println!(
                "{} config created at {}",
                agent_result.agent,
                agent_result.path.display()
            ),
            FileAction::Updated => println!(
                "{} config updated at {}",
                agent_result.agent,
                agent_result.path.display()
            ),
            FileAction::Unchanged => println!(
                "{} already configured at {}",
                agent_result.agent,
                agent_result.path.display()
            ),
        }
    }

    Ok(())
}

pub fn run_help() {
    let sections = build_help_sections();
    print!("{}", format_help_text(&sections));
}

/// Resolve the effective statement_timeout: a `--timeout` flag (validated)
/// overrides the `[settings]` value.
fn resolve_statement_timeout(cli: &Cli, settings: &Settings) -> Result<String> {
    match cli.timeout.as_deref() {
        Some(raw) => normalize_statement_timeout(raw).ok_or_else(|| {
            anyhow::anyhow!(DatabaseCliError::InvalidStatementTimeout(raw.to_string()))
        }),
        None => Ok(settings.statement_timeout.clone()),
    }
}

fn establish_connections(cli: &Cli) -> Result<(ConnectionManager, Settings)> {
    let path = resolve_config_path_with_base(cli.directory.as_deref())?;
    let configs = load_databases(&path)?;
    let settings = load_settings(&path);

    if configs.is_empty() {
        anyhow::bail!(
            "No databases configured. Create {} to add connections.",
            path.display()
        );
    }

    let timeout = resolve_statement_timeout(cli, &settings)?;
    let mut manager = ConnectionManager::with_statement_timeout(timeout);

    if cli.all_databases {
        for config in &configs {
            manager.connect(config)?;
        }
    } else if cli.databases.is_empty() {
        anyhow::bail!("Specify --db <name> or --all to select databases.");
    } else {
        for name in &cli.databases {
            let config = configs
                .iter()
                .find(|c| c.name == *name)
                .ok_or_else(|| anyhow::anyhow!("No configured database named '{name}'"))?;
            manager.connect(config)?;
        }
    }

    Ok((manager, settings))
}

pub fn run_list(cli: &Cli) -> Result<()> {
    let path = resolve_config_path_with_base(cli.directory.as_deref())?;
    let configs = load_databases(&path)?;

    if configs.is_empty() {
        println!("No databases configured.");
        println!("Create {} to add connections.", path.display());
        return Ok(());
    }

    let name_w = configs
        .iter()
        .map(|c| c.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let host_w = configs
        .iter()
        .map(|c| format!("{}:{}", c.host, c.port).len())
        .max()
        .unwrap_or(4)
        .max(4);
    let db_w = configs
        .iter()
        .map(|c| c.dbname.len())
        .max()
        .unwrap_or(8)
        .max(8);
    let user_w = configs
        .iter()
        .map(|c| c.user.len())
        .max()
        .unwrap_or(4)
        .max(4);

    println!(
        "{:<name_w$}  {:<host_w$}  {:<db_w$}  {:<user_w$}",
        "Name", "Host", "Database", "User",
    );
    println!(
        "{:-<name_w$}  {:-<host_w$}  {:-<db_w$}  {:-<user_w$}",
        "", "", "", "",
    );
    for c in &configs {
        println!(
            "{:<name_w$}  {:<host_w$}  {:<db_w$}  {:<user_w$}",
            c.name,
            format!("{}:{}", c.host, c.port),
            c.dbname,
            c.user,
        );
    }

    Ok(())
}

pub fn run_health(cli: &Cli) -> Result<()> {
    let path = resolve_config_path_with_base(cli.directory.as_deref())?;
    let configs = load_databases(&path)?;

    if configs.is_empty() {
        println!("No databases configured.");
        println!("Create {} to add connections.", path.display());
        return Ok(());
    }

    let results = check_all_health(&configs);
    print!("{}", format_health_table(&results));

    Ok(())
}

pub fn run_list_databases(cli: &Cli) -> Result<()> {
    let (mut manager, _) = establish_connections(cli)?;
    let databases = list_connected(&mut manager);
    print!("{}", format_connected_table(&databases));
    Ok(())
}

pub fn run_health_check(cli: &Cli) -> Result<()> {
    let (mut manager, _) = establish_connections(cli)?;
    let results = check_all_enhanced_health(&mut manager);
    print!("{}", format_enhanced_health_table(&results));
    Ok(())
}

pub fn run_schema(cli: &Cli, schema: &str) -> Result<()> {
    let (mut manager, _) = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = dump_schema(conn, Some(schema))?;
        print!("{}", format_schema(&result));
    }
    Ok(())
}

pub fn run_query(
    cli: &Cli,
    sql: &str,
    limit: Option<u32>,
    format: &str,
    no_header: bool,
) -> Result<()> {
    let fmt = OutputFormat::parse(format).map_err(|e| anyhow::anyhow!(e))?;
    let (mut manager, settings) = establish_connections(cli)?;
    let effective_limit = limit.unwrap_or(settings.query_limit);
    let multi = manager.len() > 1;
    for (name, conn) in manager.iter_mut() {
        let result = execute_query(conn, sql, Some(effective_limit))?;
        // Keep stdout pure data; the per-database banner follows the data
        // stream (stdout for the table view, stderr for machine formats).
        if multi {
            if fmt == OutputFormat::Table {
                println!("=== {} ===", name);
            } else {
                eprintln!("=== {} ===", name);
            }
        }
        print!("{}", format_query_data(&result, fmt, !no_header));
        eprint!("{}", format_query_summary(&result));
    }
    Ok(())
}

pub fn run_export(
    cli: &Cli,
    table: Option<&str>,
    query: Option<&str>,
    format: &str,
    output: Option<&str>,
    schema: &str,
) -> Result<()> {
    use std::io::{BufWriter, Write, stdout};

    let fmt = ExportFormat::parse(format)?;
    let request: ExportRequest = match (table, query) {
        (Some(t), None) => table_request(schema, t, fmt)?,
        (None, Some(q)) => query_request(q, fmt)?,
        (Some(_), Some(_)) => anyhow::bail!("provide either a table or --query, not both"),
        (None, None) => anyhow::bail!("provide a table name or --query <SQL>"),
    };

    let (mut manager, _settings) = establish_connections(cli)?;
    if manager.len() != 1 {
        anyhow::bail!("`export` requires exactly one --db <name>");
    }
    let (_, conn) = manager
        .iter_mut()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no active connection"))?;

    match output {
        Some(path) => {
            let file = std::fs::File::create(path)
                .map_err(|e| anyhow::anyhow!("failed to create `{path}`: {e}"))?;
            let mut w = BufWriter::new(file);
            let n = export(conn, &request, &mut w)?;
            w.flush()?;
            eprintln!("Exported {n} row(s) to {path}");
        }
        None => {
            let so = stdout();
            let mut w = BufWriter::new(so.lock());
            let n = export(conn, &request, &mut w)?;
            w.flush()?;
            eprintln!("Exported {n} row(s)");
        }
    }
    Ok(())
}

pub fn run_exec(
    cli: &Cli,
    sql: Option<&str>,
    file: Option<&str>,
    transaction: bool,
    yes: bool,
) -> Result<()> {
    if cli.all_databases {
        anyhow::bail!("`exec` requires exactly one --db <name>; --all is not supported");
    }
    let db_name = match cli.databases.as_slice() {
        [name] => name.clone(),
        [] => anyhow::bail!("`exec` requires exactly one --db <name>"),
        _ => anyhow::bail!(
            "`exec` accepts only one --db <name>; got {}",
            cli.databases.len()
        ),
    };

    match (sql, file) {
        (Some(sql), None) => {
            if transaction {
                anyhow::bail!("`--transaction` is only valid with `--file`");
            }
            run_exec_inline(cli, &db_name, sql, yes)
        }
        (None, Some(file_path)) => run_exec_file(cli, &db_name, file_path, transaction, yes),
        (Some(_), Some(_)) => {
            anyhow::bail!("provide either inline SQL or `--file <PATH>`, not both")
        }
        (None, None) => anyhow::bail!("provide SQL inline or via `--file <PATH>`"),
    }
}

fn run_exec_inline(cli: &Cli, db_name: &str, sql: &str, yes: bool) -> Result<()> {
    use std::io::IsTerminal;

    let path = resolve_config_path_with_base(cli.directory.as_deref())?;
    let configs = load_databases(&path)?;
    let settings = load_settings(&path);
    let timeout = resolve_statement_timeout(cli, &settings)?;
    let config = configs
        .iter()
        .find(|c| c.name == db_name)
        .ok_or_else(|| anyhow::anyhow!("No configured database named '{db_name}'"))?;

    let normalized = validate_single_statement(sql)?;
    match normalized.kind() {
        StatementKind::Read => anyhow::bail!(
            "`exec` is for write/DDL statements only. For read-only SQL use `databasecli query`."
        ),
        StatementKind::Unsupported => anyhow::bail!(
            "leading keyword `{}` is not supported by `exec` v1",
            normalized.first_keyword
        ),
        StatementKind::Destructive if !yes => {
            if !std::io::stdin().is_terminal() {
                return Err(anyhow::anyhow!(DatabaseCliError::ExecConfirmationRequired));
            }
            let summary = format!(
                "  {} statement: {}",
                describe_verb(&normalized.effective_verb, &normalized.first_keyword),
                normalized.sql
            );
            if !prompt_destructive_confirmation(db_name, &[summary])? {
                println!("Cancelled.");
                return Ok(());
            }
        }
        StatementKind::Destructive | StatementKind::Write => {}
    }

    let mut conn = connect_for_local_exec(config, &timeout)?;
    let result = execute_normalized(&mut conn, &normalized)?;
    print!("{}", format_execute_result(&result));
    Ok(())
}

fn run_exec_file(
    cli: &Cli,
    db_name: &str,
    file_path: &str,
    transaction: bool,
    yes: bool,
) -> Result<()> {
    use std::io::IsTerminal;

    let script_text = std::fs::read_to_string(file_path)
        .map_err(|e| anyhow::anyhow!("failed to read `{file_path}`: {e}"))?;

    let path = resolve_config_path_with_base(cli.directory.as_deref())?;
    let configs = load_databases(&path)?;
    let settings = load_settings(&path);
    let timeout = resolve_statement_timeout(cli, &settings)?;
    let config = configs
        .iter()
        .find(|c| c.name == db_name)
        .ok_or_else(|| anyhow::anyhow!("No configured database named '{db_name}'"))?;

    let mut statements = split_script(&script_text)?;

    // Reject obviously-unsupported and read-only chunks up front with line
    // context so the operator can fix the file without round-tripping through
    // a half-executed run.
    for entry in &statements {
        match entry.statement.kind() {
            StatementKind::Read => anyhow::bail!(
                "line {}: read-only SQL is not supported by `exec`. Use `databasecli query` for SELECT chains.",
                entry.start_line
            ),
            StatementKind::Unsupported => anyhow::bail!(
                "line {}: leading keyword `{}` is not supported by `exec` v1",
                entry.start_line,
                entry.statement.first_keyword
            ),
            StatementKind::Write | StatementKind::Destructive => {}
        }
    }

    if transaction {
        statements = wrap_in_transaction(statements);
    }

    let destructive: Vec<String> = statements
        .iter()
        .filter(|s| s.statement.kind() == StatementKind::Destructive)
        .map(|s| {
            format!(
                "  line {}: {} — {}",
                s.start_line,
                describe_verb(&s.statement.effective_verb, &s.statement.first_keyword),
                truncate_for_display(&s.statement.sql, 120)
            )
        })
        .collect();

    if !destructive.is_empty() && !yes {
        if !std::io::stdin().is_terminal() {
            return Err(anyhow::anyhow!(DatabaseCliError::ExecConfirmationRequired));
        }
        if !prompt_destructive_confirmation(db_name, &destructive)? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let mut conn = connect_for_local_exec(config, &timeout)?;
    let results = execute_script(&mut conn, &statements)?;
    print!("{}", format_script_results(&statements, &results));
    Ok(())
}

fn wrap_in_transaction(mut statements: Vec<ScriptStatement>) -> Vec<ScriptStatement> {
    // Build BEGIN/COMMIT ScriptStatement entries that go through the same
    // validator the operator's statements did, so the executor treats them
    // identically. line=0 marks them as injected so result formatting can
    // distinguish them from operator-written lines if desired later.
    let begin = validate_single_statement("BEGIN").expect("BEGIN validates");
    let commit = validate_single_statement("COMMIT").expect("COMMIT validates");
    let mut out = Vec::with_capacity(statements.len() + 2);
    out.push(ScriptStatement {
        statement: begin,
        start_line: 0,
    });
    out.append(&mut statements);
    out.push(ScriptStatement {
        statement: commit,
        start_line: 0,
    });
    out
}

fn describe_verb(effective: &str, first: &str) -> String {
    if effective == first {
        effective.to_string()
    } else {
        format!("{first} → {effective}")
    }
}

fn truncate_for_display(s: &str, max: usize) -> String {
    let collapsed: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    if collapsed.chars().count() <= max {
        collapsed
    } else {
        let head: String = collapsed.chars().take(max).collect();
        format!("{head}…")
    }
}

fn prompt_destructive_confirmation(db: &str, items: &[String]) -> Result<bool> {
    use std::io::{Write, stdin, stdout};
    if items.len() == 1 {
        println!("About to run a destructive statement on {db}:");
    } else {
        println!(
            "About to run {} destructive statements on {db}:",
            items.len()
        );
    }
    for item in items {
        println!("{item}");
    }
    print!("This will modify the database. Proceed? [y/N] ");
    stdout().flush()?;
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    Ok(matches!(input.trim(), "y" | "Y"))
}

pub fn run_sample(
    cli: &Cli,
    table: &str,
    schema: &str,
    limit: i64,
    order_by: Option<&str>,
) -> Result<()> {
    let (mut manager, _) = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = sample_table(conn, table, Some(schema), Some(limit), order_by)?;
        print!("{}", format_sample(&result));
    }
    Ok(())
}

pub fn run_compare(cli: &Cli, sql: &str) -> Result<()> {
    let (mut manager, settings) = establish_connections(cli)?;
    let result = compare_query(&mut manager, sql, Some(settings.query_limit))?;
    print!("{}", format_compare_result(&result));
    Ok(())
}

pub fn run_trend(
    cli: &Cli,
    table: &str,
    timestamp: &str,
    interval: &str,
    value: Option<&str>,
    schema: &str,
    limit: Option<i64>,
) -> Result<()> {
    let interval = TrendInterval::parse_interval(interval)?;
    let params = TrendParams {
        table: table.to_string(),
        schema: schema.to_string(),
        timestamp_column: timestamp.to_string(),
        interval,
        value_column: value.map(|s| s.to_string()),
        limit,
    };

    let (mut manager, _) = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = compute_trend(conn, &params)?;
        print!("{}", format_trend(&result));
    }
    Ok(())
}

pub fn run_analyze(cli: &Cli, table: &str, schema: &str) -> Result<()> {
    let (mut manager, _) = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = analyze_table(conn, table, Some(schema))?;
        print!("{}", format_table_profile(&result));
    }
    Ok(())
}

pub fn run_summary(cli: &Cli) -> Result<()> {
    let (mut manager, _) = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = summarize(conn)?;
        print!("{}", format_summary(&result));
    }
    Ok(())
}

pub fn run_erd(cli: &Cli, schema: &str, format: &str, output: Option<&str>) -> Result<()> {
    let (mut manager, _) = establish_connections(cli)?;
    let mut all_output = String::new();
    for (_, conn) in manager.iter_mut() {
        let result = build_erd(conn, Some(schema))?;
        let formatted = match format {
            "mermaid" => format_erd_mermaid(&result),
            "dot" => format_erd_dot(&result),
            _ => format_erd_ascii(&result),
        };
        all_output.push_str(&formatted);
    }

    if let Some(path) = output {
        std::fs::write(path, &all_output)?;
        println!("ERD written to {path}");
    } else {
        print!("{all_output}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Cli;
    use clap::Parser;

    fn cli_with(args: &[&str]) -> Cli {
        let mut full = vec!["databasecli"];
        full.extend_from_slice(args);
        Cli::parse_from(full)
    }

    #[test]
    fn exec_rejects_all_flag() {
        let cli = cli_with(&["--all", "exec", "DELETE FROM t"]);
        let err = run_exec(&cli, Some("DELETE FROM t"), None, false, false).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("--all is not supported"),
            "expected --all rejection, got: {msg}"
        );
    }

    #[test]
    fn exec_rejects_zero_db() {
        let cli = cli_with(&["exec", "DELETE FROM t"]);
        let err = run_exec(&cli, Some("DELETE FROM t"), None, false, false).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("requires exactly one --db"),
            "expected zero-db rejection, got: {msg}"
        );
    }

    #[test]
    fn exec_rejects_multiple_dbs() {
        let cli = cli_with(&["--db", "a", "--db", "b", "exec", "DELETE FROM t"]);
        let err = run_exec(&cli, Some("DELETE FROM t"), None, false, false).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("only one --db"),
            "expected multi-db rejection, got: {msg}"
        );
    }

    #[test]
    fn exec_yes_flag_parses() {
        let cli = cli_with(&["--db", "a", "exec", "--yes", "DELETE FROM t"]);
        let err = run_exec(&cli, Some("DELETE FROM t"), None, false, true).unwrap_err();
        let msg = err.to_string();
        assert!(
            !msg.contains("requires exactly one --db") && !msg.contains("--all"),
            "selection rules should pass with one --db; got: {msg}"
        );
    }

    #[test]
    fn exec_rejects_both_inline_sql_and_file() {
        let cli = cli_with(&["--db", "a", "exec", "DELETE FROM t"]);
        let err = run_exec(
            &cli,
            Some("DELETE FROM t"),
            Some("/tmp/x.sql"),
            false,
            false,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("not both"),
            "expected mutual-exclusion error, got: {msg}"
        );
    }

    #[test]
    fn exec_rejects_neither_inline_sql_nor_file() {
        let cli = cli_with(&["--db", "a", "exec", ""]);
        let err = run_exec(&cli, None, None, false, false).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("provide SQL inline or via `--file`") || msg.contains("--file"),
            "expected missing-input error, got: {msg}"
        );
    }

    #[test]
    fn exec_rejects_transaction_flag_without_file() {
        let cli = cli_with(&["--db", "a", "exec", "DELETE FROM t"]);
        let err = run_exec(&cli, Some("DELETE FROM t"), None, true, true).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("--transaction"),
            "expected --transaction rejection, got: {msg}"
        );
    }

    #[test]
    fn exec_file_arg_parses_via_clap() {
        // `requires = "file"` and `conflicts_with = "sql"` on the clap attrs
        // are exercised here — this only checks the parser accepts the form.
        let cli = cli_with(&[
            "--db",
            "a",
            "exec",
            "--file",
            "/tmp/seed.sql",
            "--transaction",
            "--yes",
        ]);
        // Pull the parsed fields out and assert; we don't run anything that
        // would need a real file.
        match cli.command {
            Some(crate::args::Commands::Exec {
                sql,
                file,
                transaction,
                yes,
            }) => {
                assert!(sql.is_none());
                assert_eq!(file.as_deref(), Some("/tmp/seed.sql"));
                assert!(transaction);
                assert!(yes);
            }
            _ => panic!("expected Exec subcommand"),
        }
    }
}
