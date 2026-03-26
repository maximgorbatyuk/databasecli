mod args;
mod run;

use anyhow::Result;
use clap::Parser;

use args::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Tui) | None => databasecli_tui::run()?,
        Some(Commands::List) => run::run_list()?,
        Some(Commands::Health) => run::run_health()?,
    }

    Ok(())
}
