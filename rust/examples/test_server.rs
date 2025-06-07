use anytype_mcp_rust::config::Config;
use anytype_mcp_rust::openapi::McpTool;
use anytype_mcp_rust::server::AnytypeMcpServer;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Testing MCP server with tool registration...");

    // Create a minimal config
    let config = Config {
        spec_path: None,
        base_url: Some("http://localhost:31009".to_string()),
        headers: Default::default(),
        timeout_seconds: Some(30),
        max_retries: Some(3),
    };

    // Create a test tool manually to verify our conversion logic
    let test_tool = McpTool {
        name: "test_operation".to_string(),
        description: Some("A test tool for verification".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name parameter"
                },
                "count": {
                    "type": "number",
                    "description": "The count parameter"
                }
            },
            "required": ["name"]
        }),
        method: "GET".to_string(),
        path: "/test".to_string(),
        operation_id: "test_operation".to_string(),
    };

    println!("Created test tool: {:?}", test_tool);

    // Test the schema conversion function
    let converted_schema = AnytypeMcpServer::convert_schema_to_tool_input(&test_tool.input_schema);

    println!("Converted schema: {:?}", converted_schema);
    println!("✅ Schema conversion test successful!");

    Ok(())
}
