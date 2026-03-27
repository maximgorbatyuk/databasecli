mod json_convert;
mod server;
mod state;
mod tools;

use anyhow::Result;
use clap::Parser;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

use databasecli_core::config::{create_default_config, resolve_config_path_with_base};
use server::DatabaseCliServer;
use state::McpSessionState;

#[derive(Parser)]
#[command(
    name = "databasecli-mcp",
    about = "MCP server for PostgreSQL via databasecli",
    version
)]
struct Args {
    /// Working directory for config resolution
    #[arg(short = 'D', long = "directory")]
    directory: Option<String>,

    /// Create a template databases.ini config file and exit
    #[arg(long)]
    init: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.init {
        let path = resolve_config_path_with_base(args.directory.as_deref())?;
        if path.exists() {
            println!("Config already exists at {}", path.display());
            println!("Edit it to add your database connections.");
        } else {
            create_default_config(&path)?;
            println!("Config created at {}", path.display());
            println!("Edit it to add your database connections:");
            println!();
            println!("  [my_database]");
            println!("  host = localhost");
            println!("  port = 5432");
            println!("  user = postgres");
            println!("  password = secret");
            println!("  dbname = my_database");
        }
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting databasecli MCP server");

    let state = McpSessionState::new(args.directory.as_deref())?;

    tracing::info!("Loaded {} database config(s)", state.configs.len());

    let server = DatabaseCliServer::new(state);
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;

    Ok(())
}
