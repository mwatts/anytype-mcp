use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, model::*,
    service::RequestContext, transport::stdio,
};
use serde_json::{Map, Value, json};
use tracing::{error, info};

use crate::client::HttpClient;
use crate::config::Config;
use crate::openapi::{McpTool, OpenApiParser, get_base_url, load_openapi_spec};
use crate::utils::{AnytypeMcpError, Result as McpResult};

/// JSON-RPC MCP Server that converts OpenAPI specs to MCP tools
#[derive(Clone)]
pub struct AnytypeJsonRpcServer {
    #[allow(dead_code)]
    config: Arc<Config>,
    http_client: Arc<HttpClient>,
    tools: Arc<Vec<McpTool>>,
    tool_map: Arc<HashMap<String, McpTool>>,
}

impl AnytypeJsonRpcServer {
    /// Create a new JSON-RPC MCP server
    pub async fn new(spec_path: Option<String>, config: Config) -> McpResult<Self> {
        info!("Initializing Anytype JSON-RPC MCP Server");

        // Embedded OpenAPI spec as fallback
        const EMBEDDED_OPENAPI_SPEC: &str = include_str!("../../../scripts/openapi.json");

        // Load OpenAPI spec with priority order
        let spec = if let Some(path) = spec_path.or_else(|| config.spec_path.clone()) {
            info!("Loading OpenAPI specification from: {}", path);
            if path.starts_with("http") {
                // Download from URL
                let response = reqwest::get(&path)
                    .await
                    .map_err(AnytypeMcpError::HttpClient)?;
                let content = response.text().await.map_err(AnytypeMcpError::HttpClient)?;
                serde_json::from_str(&content).map_err(AnytypeMcpError::Json)?
            } else {
                load_openapi_spec(&path).await?
            }
        } else if std::path::Path::new("scripts/openapi.json").exists() {
            info!("Loading OpenAPI specification from: scripts/openapi.json");
            load_openapi_spec("scripts/openapi.json").await?
        } else if std::path::Path::new("openapi.json").exists() {
            info!("Loading OpenAPI specification from: openapi.json");
            load_openapi_spec("openapi.json").await?
        } else {
            // Try embedded spec before falling back to remote URL
            info!("Using embedded OpenAPI specification");
            match serde_json::from_str(EMBEDDED_OPENAPI_SPEC) {
                Ok(spec) => spec,
                Err(_) => {
                    info!("Failed to parse embedded spec, falling back to remote URL");
                    let response = reqwest::get("https://api.anytype.io/openapi.json")
                        .await
                        .map_err(AnytypeMcpError::HttpClient)?;
                    let content = response.text().await.map_err(AnytypeMcpError::HttpClient)?;
                    serde_json::from_str(&content).map_err(AnytypeMcpError::Json)?
                }
            }
        };

        // Get base URL
        let base_url = config
            .base_url
            .clone()
            .or_else(|| get_base_url(&spec))
            .unwrap_or_else(|| crate::config::DEFAULT_BASE_URL.to_string());

        info!("Using base URL: {}", base_url);

        // Initialize HTTP client
        let http_client = HttpClient::new(&config, base_url)?;

        // Convert OpenAPI spec to MCP tools
        let parser = OpenApiParser::new(spec);
        parser.validate()?;
        let tools = parser.convert_to_tools()?;

        info!("Converted {} OpenAPI operations to MCP tools", tools.len());

        // Create tool map for quick lookup
        let tool_map: HashMap<String, McpTool> = tools
            .iter()
            .map(|tool| (tool.name.clone(), tool.clone()))
            .collect();

        Ok(Self {
            config: Arc::new(config),
            http_client: Arc::new(http_client),
            tools: Arc::new(tools),
            tool_map: Arc::new(tool_map),
        })
    }

    /// Start the server with stdio transport
    pub async fn start_stdio(self) -> Result<()> {
        info!("Starting JSON-RPC MCP server with stdio transport");

        let service = self.serve(stdio()).await.map_err(|e| {
            error!("Failed to start stdio transport: {:?}", e);
            anyhow::anyhow!("Failed to start stdio transport: {:?}", e)
        })?;

        info!("MCP server started successfully with stdio transport");
        service.waiting().await.map_err(|e| {
            error!("Server error: {:?}", e);
            anyhow::anyhow!("Server error: {:?}", e)
        })?;

        Ok(())
    }

    /// Get server information
    pub fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "anytype-mcp-server",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(format!(
                "This server provides {} tools converted from an OpenAPI specification. Each tool corresponds to an API endpoint that can be called.",
                self.tools.len()
            ))
    }

    /// Get the list of tools
    pub fn get_tools(&self) -> &Vec<McpTool> {
        &self.tools
    }

    /// Get the HTTP client
    #[allow(dead_code)]
    pub fn get_http_client(&self) -> &HttpClient {
        &self.http_client
    }

    /// Convert OpenAPI schema to MCP tool input schema
    pub fn convert_schema_to_tool_input(schema: &Value) -> Value {
        // Convert OpenAPI schema format to MCP tool input format
        if let Some(obj) = schema.as_object() {
            let mut converted = json!({
                "type": obj.get("type").unwrap_or(&json!("object")).as_str().unwrap_or("object")
            });

            // Handle properties
            if let Some(properties) = obj.get("properties") {
                converted["properties"] = properties.clone();
            }

            // Handle required fields
            if let Some(required) = obj.get("required") {
                converted["required"] = required.clone();
            }

            // Handle array items
            if let Some(items) = obj.get("items") {
                converted["items"] = Self::convert_schema_to_tool_input(items);
            }

            converted
        } else {
            // If not an object, return as-is or default to object type
            schema.clone()
        }
    }
}

impl ServerHandler for AnytypeJsonRpcServer {
    fn get_info(&self) -> ServerInfo {
        self.get_info()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools: Vec<Tool> = self
            .tools
            .iter()
            .map(|mcp_tool| {
                Tool::new_with_raw(
                    mcp_tool.name.clone(),
                    mcp_tool.description.clone().map(Into::into),
                    Arc::new(
                        Self::convert_schema_to_tool_input(&mcp_tool.input_schema)
                            .as_object()
                            .unwrap()
                            .clone(),
                    ),
                )
            })
            .collect();

        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let CallToolRequestParams {
            name, arguments, ..
        } = request;
        info!("Calling tool: {}", name);

        // Find the tool - convert name to string for lookup
        let tool = self
            .tool_map
            .get(name.as_ref())
            .ok_or_else(|| McpError::invalid_params("Tool not found", None))?;

        // Execute the tool using the HTTP client
        let args = arguments.unwrap_or_else(Map::new);
        let args_value = Value::Object(args);

        match self.http_client.execute_tool(tool, args_value).await {
            Ok(result) => {
                info!("Tool '{}' executed successfully", name);
                Ok(CallToolResult::success(vec![ContentBlock::text(
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()),
                )])
                .into())
            }
            Err(e) => {
                error!("Tool '{}' execution failed: {:?}", name, e);
                Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                    "Tool execution failed: {}",
                    e
                ))])
                .into())
            }
        }
    }
}
