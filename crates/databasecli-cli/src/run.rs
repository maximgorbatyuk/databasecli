use anyhow::Result;
use databasecli_core::commands::analyze::{analyze_table, format_table_profile};
use databasecli_core::commands::compare::{compare_query, format_compare_result};
use databasecli_core::commands::erd::{
    build_erd, format_erd_ascii, format_erd_dot, format_erd_mermaid,
};
use databasecli_core::commands::health::{check_all_enhanced_health, format_enhanced_health_table};
use databasecli_core::commands::list_databases::{format_connected_table, list_connected};
use databasecli_core::commands::query::{execute_query, format_query_result};
use databasecli_core::commands::sample::{format_sample, sample_table};
use databasecli_core::commands::schema::{dump_schema, format_schema};
use databasecli_core::commands::summary::{format_summary, summarize};
use databasecli_core::commands::trend::{TrendInterval, TrendParams, compute_trend, format_trend};
use databasecli_core::config::{
    load_databases, resolve_config_path, resolve_config_path_with_base,
};
use databasecli_core::connection::ConnectionManager;
use databasecli_core::health::{check_all_health, format_health_table};

use databasecli_core::help::{build_help_sections, format_help_text};

use crate::args::Cli;

pub fn run_help() {
    let sections = build_help_sections();
    print!("{}", format_help_text(&sections));
}

fn establish_connections(cli: &Cli) -> Result<ConnectionManager> {
    let path = resolve_config_path_with_base(cli.directory.as_deref())?;
    let configs = load_databases(&path)?;

    if configs.is_empty() {
        anyhow::bail!(
            "No databases configured. Create {} to add connections.",
            path.display()
        );
    }

    let mut manager = ConnectionManager::new();

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

    Ok(manager)
}

pub fn run_list() -> Result<()> {
    let path = resolve_config_path()?;
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

pub fn run_health() -> Result<()> {
    let path = resolve_config_path()?;
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
    let mut manager = establish_connections(cli)?;
    let databases = list_connected(&mut manager);
    print!("{}", format_connected_table(&databases));
    Ok(())
}

pub fn run_health_check(cli: &Cli) -> Result<()> {
    let mut manager = establish_connections(cli)?;
    let results = check_all_enhanced_health(&mut manager);
    print!("{}", format_enhanced_health_table(&results));
    Ok(())
}

pub fn run_schema(cli: &Cli, schema: &str) -> Result<()> {
    let mut manager = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = dump_schema(conn, Some(schema))?;
        print!("{}", format_schema(&result));
    }
    Ok(())
}

pub fn run_query(cli: &Cli, sql: &str) -> Result<()> {
    let mut manager = establish_connections(cli)?;
    let multi = manager.len() > 1;
    for (name, conn) in manager.iter_mut() {
        let result = execute_query(conn, sql)?;
        if multi {
            println!("=== {} ===", name);
        }
        print!("{}", format_query_result(&result));
    }
    Ok(())
}

pub fn run_sample(
    cli: &Cli,
    table: &str,
    schema: &str,
    limit: i64,
    order_by: Option<&str>,
) -> Result<()> {
    let mut manager = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = sample_table(conn, table, Some(schema), Some(limit), order_by)?;
        print!("{}", format_sample(&result));
    }
    Ok(())
}

pub fn run_compare(cli: &Cli, sql: &str) -> Result<()> {
    let mut manager = establish_connections(cli)?;
    let result = compare_query(&mut manager, sql)?;
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

    let mut manager = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = compute_trend(conn, &params)?;
        print!("{}", format_trend(&result));
    }
    Ok(())
}

pub fn run_analyze(cli: &Cli, table: &str, schema: &str) -> Result<()> {
    let mut manager = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = analyze_table(conn, table, Some(schema))?;
        print!("{}", format_table_profile(&result));
    }
    Ok(())
}

pub fn run_summary(cli: &Cli) -> Result<()> {
    let mut manager = establish_connections(cli)?;
    for (_, conn) in manager.iter_mut() {
        let result = summarize(conn)?;
        print!("{}", format_summary(&result));
    }
    Ok(())
}

pub fn run_erd(cli: &Cli, schema: &str, format: &str, output: Option<&str>) -> Result<()> {
    let mut manager = establish_connections(cli)?;
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
