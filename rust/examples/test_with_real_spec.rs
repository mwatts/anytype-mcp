use anytype_mcp_rust::config::Config;
use anytype_mcp_rust::server::AnytypeJsonRpcServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Testing MCP server with real OpenAPI specification...");

    let config = Config::default();

    // Test with a simple API spec
    let spec_path = "test-api.json";

    if !std::path::Path::new(spec_path).exists() {
        println!("❌ Test spec file not found: {}", spec_path);
        println!("This test requires the eBay API specification file");
        return Ok(());
    }

    println!("📖 Loading OpenAPI spec from: {}", spec_path);

    // Create server with real spec
    match AnytypeJsonRpcServer::new(Some(spec_path.to_string()), config).await {
        Ok(server) => {
            println!("✅ Server created successfully!");
            println!("🔧 Available tools: {}", server.get_tools().len());

            // Print some tool information
            for (i, tool) in server.get_tools().iter().take(3).enumerate() {
                println!("  Tool {}: {} ({})", i + 1, tool.name, tool.method);
                if let Some(desc) = &tool.description {
                    println!("    Description: {}", desc);
                }
            }

            if server.get_tools().len() > 3 {
                println!("  ... and {} more tools", server.get_tools().len() - 3);
            }

            println!("🧪 Note: JSON-RPC server would run on stdio transport");
            println!("✅ Server validation completed successfully!");
        }
        Err(e) => {
            println!("❌ Failed to create server: {}", e);
            return Err(e.into());
        }
    }

    println!("🎉 All tests completed successfully!");
    Ok(())
}
