mod args;
mod run;

use anyhow::Result;
use clap::Parser;

use args::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Tui) | None => databasecli_tui::run(cli.directory)?,
        Some(Commands::Init) => run::run_init(&cli)?,
        Some(Commands::List) => run::run_list(&cli)?,
        Some(Commands::Health) => run::run_health(&cli)?,
        Some(Commands::ListDatabases) => run::run_list_databases(&cli)?,
        Some(Commands::HealthCheck) => run::run_health_check(&cli)?,
        Some(Commands::Schema { ref schema }) => run::run_schema(&cli, schema)?,
        Some(Commands::Query { ref sql }) => run::run_query(&cli, sql)?,
        Some(Commands::Analyze {
            ref table,
            ref schema,
        }) => run::run_analyze(&cli, table, schema)?,
        Some(Commands::Summary) => run::run_summary(&cli)?,
        Some(Commands::Erd {
            ref schema,
            ref format,
            ref output,
        }) => run::run_erd(&cli, schema, format, output.as_deref())?,
        Some(Commands::Reference) => run::run_help(),
        Some(Commands::Compare { ref sql }) => run::run_compare(&cli, sql)?,
        Some(Commands::Trend {
            ref table,
            ref timestamp,
            ref interval,
            ref value,
            ref schema,
            limit,
        }) => run::run_trend(
            &cli,
            table,
            timestamp,
            interval,
            value.as_deref(),
            schema,
            limit,
        )?,
        Some(Commands::Sample {
            ref table,
            limit,
            ref order_by,
            ref schema,
        }) => run::run_sample(&cli, table, schema, limit, order_by.as_deref())?,
    }

    Ok(())
}
