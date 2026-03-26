mod json_convert;
mod server;
mod state;
mod tools;

use anyhow::Result;
use clap::Parser;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let args = Args::parse();

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
