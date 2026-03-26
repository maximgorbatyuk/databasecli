use anyhow::Result;
use databasecli_core::config::{load_databases, resolve_config_path};
use databasecli_core::health::{check_all_health, format_health_table};

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
