use clap::{Parser, Subcommand, ValueEnum};
use tracing::info;
use anyhow::Result;

mod config;
mod server;
mod openapi;
mod client;
mod auth;
mod utils;

use config::Config;
use server::{HybridMcpServer, ServerMode};
use auth::KeyGenerator;

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

    /// Server transport mode
    #[arg(long, global = true, value_enum)]
    mode: Option<TransportMode>,

    /// Port for HTTP-based transports (SSE, Streamable HTTP)
    #[arg(long, global = true, default_value = "8080")]
    port: u16,
}

#[derive(ValueEnum, Clone, Debug)]
enum TransportMode {
    /// Standard input/output (default)
    Stdio,
    /// Server-Sent Events over HTTP
    Sse,
    /// Streamable HTTP (experimental)
    StreamableHttp,
}

impl From<TransportMode> for ServerMode {
    fn from(mode: TransportMode) -> Self {
        match mode {
            TransportMode::Stdio => ServerMode::JsonRpcStdio,
            TransportMode::Sse => ServerMode::JsonRpcSse { port: 8080 }, // Default port, will be overridden
            TransportMode::StreamableHttp => ServerMode::JsonRpcStreamableHttp { port: 8080 }, // Default port, will be overridden
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Run the MCP server (default)
    Run {
        #[arg(long)]
        spec_path: Option<String>,
        #[arg(long, value_enum)]
        mode: Option<TransportMode>,
        #[arg(long)]
        port: Option<u16>,
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
        #[arg(long, value_enum)]
        mode: Option<TransportMode>,
        #[arg(long)]
        port: Option<u16>,
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
        .with_writer(std::io::stderr) // ✅ Write logs to stderr
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(log_level.parse().unwrap())
        )
        .init();

    info!("Starting Anytype MCP Server (Rust) with JSON-RPC Protocol");

    // Load configuration
    let config = Config::load(cli.spec_path.as_deref())?;

    match cli.command.unwrap_or(Commands::Run {
        spec_path: None,
        mode: None,
        port: None
    }) {
        Commands::Run { spec_path, mode, port } => {
            let final_spec_path = spec_path.or(cli.spec_path).or(config.spec_path.clone());
            let final_mode = mode.or(cli.mode).unwrap_or(TransportMode::Stdio);
            let final_port = port.unwrap_or(cli.port);
            run_server(final_spec_path, config, final_mode, final_port).await
        }
        Commands::GetKey { spec_path } => {
            let final_spec_path = spec_path.or(cli.spec_path).or(config.spec_path.clone());
            generate_api_key(final_spec_path, config).await
        }
        Commands::Validate { spec_path, mode, port } => {
            let final_spec_path = spec_path.or(cli.spec_path).or(config.spec_path.clone());
            let final_mode = mode.or(cli.mode).unwrap_or(TransportMode::Stdio);
            let final_port = port.unwrap_or(cli.port);
            validate_server(final_spec_path, config, final_mode, final_port).await
        }
        Commands::ListTools { spec_path } => {
            let final_spec_path = spec_path.or(cli.spec_path).or(config.spec_path.clone());
            list_tools(final_spec_path, config).await
        }
    }
}

async fn run_server(spec_path: Option<String>, config: Config, mode: TransportMode, port: u16) -> Result<()> {
    info!("Initializing MCP server with mode: {:?}", mode);

    let server_mode = match mode {
        TransportMode::Stdio => ServerMode::JsonRpcStdio,
        TransportMode::Sse => ServerMode::JsonRpcSse { port },
        TransportMode::StreamableHttp => ServerMode::JsonRpcStreamableHttp { port },
    };

    let server = HybridMcpServer::new(spec_path, config, server_mode);

    info!("Starting MCP server...");
    server.start().await?;

    Ok(())
}

async fn validate_server(spec_path: Option<String>, config: Config, mode: TransportMode, port: u16) -> Result<()> {
    info!("Validating MCP server configuration");

    let server_mode = match mode {
        TransportMode::Stdio => ServerMode::JsonRpcStdio,
        TransportMode::Sse => ServerMode::JsonRpcSse { port },
        TransportMode::StreamableHttp => ServerMode::JsonRpcStreamableHttp { port },
    };

    let server = HybridMcpServer::new(spec_path, config, server_mode);
    server.validate().await?;

    let info = server.get_server_info().await?;
    println!("✅ Server configuration is valid!");
    println!("{}", info);

    Ok(())
}

async fn list_tools(spec_path: Option<String>, config: Config) -> Result<()> {
    info!("Listing available tools");

    let server = HybridMcpServer::new(spec_path, config, ServerMode::JsonRpcStdio);
    let tools = server.list_tools().await?;

    if tools.is_empty() {
        println!("No tools available. Make sure an OpenAPI specification is provided.");
    } else {
        println!("Available tools ({}):", tools.len());
        for (i, tool) in tools.iter().enumerate() {
            println!("  {}. {}", i + 1, tool);
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
