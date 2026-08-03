use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

mod auth;
mod client;
mod config;
mod openapi;
mod server;
mod utils;

use auth::KeyGenerator;
use config::Config;
use server::AnytypeJsonRpcServer;

#[derive(Parser)]
#[command(name = "anytype-mcp")]
#[command(about = "Anytype MCP Server - Rust Implementation with JSON-RPC Protocol")]
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
    /// Run the MCP server on stdio (default)
    Run {
        #[arg(long)]
        spec_path: Option<String>,
    },
    /// Generate API key interactively
    GetKey {
        #[arg(long)]
        spec_path: Option<String>,
    },
    /// Validate server configuration
    Validate {
        #[arg(long)]
        spec_path: Option<String>,
    },
    /// List available tools
    ListTools {
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
        .with_writer(std::io::stderr) // stdout is reserved for the MCP protocol
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(log_level.parse().unwrap()),
        )
        .init();

    info!("Starting Anytype MCP Server (Rust) with JSON-RPC Protocol");

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
        Commands::Validate { spec_path } => {
            let final_spec_path = spec_path.or(cli.spec_path).or(config.spec_path.clone());
            validate_server(final_spec_path, config).await
        }
        Commands::ListTools { spec_path } => {
            let final_spec_path = spec_path.or(cli.spec_path).or(config.spec_path.clone());
            list_tools(final_spec_path, config).await
        }
    }
}

async fn run_server(spec_path: Option<String>, config: Config) -> Result<()> {
    let server = AnytypeJsonRpcServer::new(spec_path, config).await?;

    info!("Starting MCP server on stdio...");
    server.start_stdio().await
}

async fn validate_server(spec_path: Option<String>, config: Config) -> Result<()> {
    info!("Validating MCP server configuration");

    if let Some(path) = &spec_path {
        if !path.starts_with("http") && !std::path::Path::new(path).exists() {
            return Err(anyhow::anyhow!("OpenAPI spec file not found: {}", path));
        }
    }

    let server = AnytypeJsonRpcServer::new(spec_path, config).await?;
    let info = server.get_info();

    println!("✅ Server configuration is valid!");
    println!(
        "Server: {} v{}\nCapabilities: {:?}\nTools: {}",
        info.server_info.name,
        info.server_info.version,
        info.capabilities,
        server.get_tools().len()
    );

    Ok(())
}

async fn list_tools(spec_path: Option<String>, config: Config) -> Result<()> {
    info!("Listing available tools");

    let server = AnytypeJsonRpcServer::new(spec_path, config).await?;
    let tools = server.get_tools();

    if tools.is_empty() {
        println!("No tools available. Make sure an OpenAPI specification is provided.");
    } else {
        println!("Available tools ({}):", tools.len());
        for (i, tool) in tools.iter().enumerate() {
            println!("  {}. {}", i + 1, tool.name);
        }
    }

    Ok(())
}

async fn generate_api_key(spec_path: Option<String>, config: Config) -> Result<()> {
    info!("Starting API key generation");

    let key_generator = KeyGenerator::new(spec_path, config).await?;
    key_generator.generate_interactive().await?;

    Ok(())
}
