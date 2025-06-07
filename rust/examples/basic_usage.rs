use anytype_mcp_rust::{Config, AnytypeMcpServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().init();

    // Load configuration
    let config = Config::default();

    // Create server
    let server = AnytypeMcpServer::new(
        Some("../scripts/openapi.json".to_string()),
        config
    ).await?;

    println!("Created server with {} tools", server.get_tools().len());

    // List available tools
    for tool in server.get_tools() {
        println!("Tool: {} - {}", tool.name, tool.description.as_deref().unwrap_or("No description"));
    }

    Ok(())
}
