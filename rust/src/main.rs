use clap::{Parser, Subcommand};
use tracing::info;
use anyhow::Result;

mod config;
mod server;
mod openapi;
mod client;
mod auth;
mod utils;

use config::Config;
use server::AnytypeMcpServer;
use auth::KeyGenerator;

#[derive(Parser)]
#[command(name = "anytype-mcp")]
#[command(about = "Anytype MCP Server - Rust Implementation")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// OpenAPI specification file path
    #[arg(long, global = true)]
    spec_path: Option<String>,

    /// Enable debug logging
    #[arg(long, global = true)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the MCP server (default)
    Run {
        #[arg(long)]
        spec_path: Option<String>,
    },
    /// Generate API key interactively
    GetKey {
        #[arg(long)]
        spec_path: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(log_level.parse().unwrap())
        )
        .init();

    info!("Starting Anytype MCP Server (Rust)");

    // Load configuration
    let config = Config::load(cli.spec_path.as_deref())?;

    match cli.command.unwrap_or(Commands::Run { spec_path: None }) {
        Commands::Run { spec_path } => {
            let final_spec_path = spec_path.or(cli.spec_path).or(config.spec_path.clone());
            run_server(final_spec_path, config).await
        }
        Commands::GetKey { spec_path } => {
            let final_spec_path = spec_path.or(cli.spec_path).or(config.spec_path.clone());
            generate_api_key(final_spec_path, config).await
        }
    }
}

async fn run_server(spec_path: Option<String>, config: Config) -> Result<()> {
    info!("Initializing MCP server");

    let mut server = AnytypeMcpServer::new(spec_path, config).await?;

    info!("Starting MCP server with stdio transport");
    server.start().await?;

    Ok(())
}

async fn generate_api_key(spec_path: Option<String>, config: Config) -> Result<()> {
    info!("Starting API key generation");

    let key_generator = KeyGenerator::new(spec_path, config).await?;
    key_generator.generate_interactive().await?;

    Ok(())
}
