use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "databasecli",
    about = "PostgreSQL database connection manager",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Launch the interactive TUI
    Tui,
    /// List all stored database connections
    List,
    /// Check health of all stored database connections
    Health,
}
