use std::collections::HashMap;

use mcpr::{
    transport::stdio::StdioTransport,
    server::{Server, ServerConfig},
};
use tracing::info;

use crate::config::Config;
use crate::client::HttpClient;
use crate::openapi::{load_openapi_spec, get_base_url, OpenApiParser, McpTool};
use crate::utils::{AnytypeMcpError, Result as McpResult};

pub struct AnytypeMcpServer {
    #[allow(dead_code)]
    config: Config,
    #[allow(dead_code)]
    http_client: HttpClient,
    tools: Vec<McpTool>,
    tool_map: HashMap<String, McpTool>,
}

impl AnytypeMcpServer {
    pub async fn new(spec_path: Option<String>, config: Config) -> McpResult<Self> {
        info!("Initializing Anytype MCP Server");

        // Determine spec path
        let spec_path = spec_path
            .or_else(|| config.spec_path.clone())
            .unwrap_or_else(|| {
                // Default to bundled spec or common paths
                if std::path::Path::new("scripts/openapi.json").exists() {
                    "scripts/openapi.json".to_string()
                } else if std::path::Path::new("openapi.json").exists() {
                    "openapi.json".to_string()
                } else {
                    "https://api.anytype.io/openapi.json".to_string()
                }
            });

        info!("Loading OpenAPI specification from: {}", spec_path);

        // Load OpenAPI spec
        let spec = if spec_path.starts_with("http") {
            // Download from URL
            let response = reqwest::get(&spec_path).await
                .map_err(AnytypeMcpError::HttpClient)?;
            let content = response.text().await
                .map_err(AnytypeMcpError::HttpClient)?;
            serde_json::from_str(&content)
                .map_err(AnytypeMcpError::Json)?
        } else {
            load_openapi_spec(&spec_path).await?
        };

        // Get base URL
        let base_url = config.base_url.clone()
            .or_else(|| get_base_url(&spec))
            .unwrap_or_else(|| "http://localhost:31009".to_string());

        info!("Using base URL: {}", base_url);

        // Initialize HTTP client
        let http_client = HttpClient::new(&config, base_url)?;

        // Convert OpenAPI spec to MCP tools
        let parser = OpenApiParser::new(spec);
        parser.validate()?;
        let tools = parser.convert_to_tools()?;

        info!("Converted {} OpenAPI operations to MCP tools", tools.len());

        // Create tool map for quick lookup
        let tool_map: HashMap<String, McpTool> = tools.iter()
            .map(|tool| (tool.name.clone(), tool.clone()))
            .collect();

        Ok(Self {
            config,
            http_client,
            tools,
            tool_map,
        })
    }

    pub async fn start(&mut self) -> McpResult<()> {
        info!("Starting MCP server");

        // Create server configuration
        let server_config = ServerConfig::new()
            .with_name("Anytype MCP Server")
            .with_version("1.0.0");
         // For now, skip tool registration and just create a basic server
        // TODO: Fix ToolInputSchema conversion and re-enable tool registration
        info!("Creating MCPR server (tools will be added in next iteration)");

        // Create server
        let mut server = Server::new(server_config);

        info!("MCPR server created, skipping tool registration for now");

        /*
        // TODO: Fix this section once ToolInputSchema conversion is resolved
        // Convert tools to MCPR format with simplified schema
        let mcpr_tools: Vec<Tool> = self.tools.iter().map(|tool| {
            Tool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(), // Fix: proper conversion needed
            }
        }).collect();

        // Add tools to config
        let server_config = mcpr_tools.into_iter()
            .fold(server_config, |config, tool| config.with_tool(tool));

        // Create shared state for the server
        let http_client = Arc::new(self.http_client.clone());
        let tool_map = Arc::new(self.tool_map.clone());

        // Register tool handlers
        for tool in &self.tools {
            let tool_name = tool.name.clone();
            let client = Arc::clone(&http_client);
            let tools = Arc::clone(&tool_map);

            server.register_tool_handler(&tool_name, move |params: Value| {
                let client = Arc::clone(&client);
                let tools = Arc::clone(&tools);
                let tool_name = tool_name.clone();

                debug!("Executing tool: {}", tool_name);

                if let Some(tool) = tools.get(&tool_name) {
                    // Use tokio to block on the async operation
                    match tokio::runtime::Handle::current().block_on(
                        client.execute_tool(tool, params)
                    ) {
                        Ok(result) => Ok(result),
                        Err(e) => {
                            error!("Tool execution failed: {}", e);
                            Ok(serde_json::json!({
                                "error": e.to_string()
                            }))
                        }
                    }
                } else {
                    error!("Tool not found: {}", tool_name);
                    Ok(serde_json::json!({
                        "error": format!("Tool not found: {}", tool_name)
                    }))
                }
            })?;
        }

        info!("Registered {} tool handlers", self.tools.len());
        */

        // Start server with stdio transport
        let transport = StdioTransport::new();
        server.start(transport)
            .map_err(|e| AnytypeMcpError::McpProtocol(e.to_string()))?;

        Ok(())
    }

    pub fn get_tools(&self) -> &[McpTool] {
        &self.tools
    }

    pub fn get_tool(&self, name: &str) -> Option<&McpTool> {
        self.tool_map.get(name)
    }
}
