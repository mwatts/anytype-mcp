use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    Error as McpError, RoleServer, ServerHandler, model::*,
    service::RequestContext, transport::stdio,
    ServiceExt,
};
use serde_json::{json, Value, Map};
use tracing::{info, error};

use crate::config::Config;
use crate::client::HttpClient;
use crate::openapi::{load_openapi_spec, get_base_url, OpenApiParser, McpTool};
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
                let response = reqwest::get(&path).await
                    .map_err(AnytypeMcpError::HttpClient)?;
                let content = response.text().await
                    .map_err(AnytypeMcpError::HttpClient)?;
                serde_json::from_str(&content)
                    .map_err(AnytypeMcpError::Json)?
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
                    let response = reqwest::get("https://api.anytype.io/openapi.json").await
                        .map_err(AnytypeMcpError::HttpClient)?;
                    let content = response.text().await
                        .map_err(AnytypeMcpError::HttpClient)?;
                    serde_json::from_str(&content)
                        .map_err(AnytypeMcpError::Json)?
                }
            }
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

    /// Start the server with SSE transport
    pub async fn start_sse(self, port: u16) -> Result<()> {
        info!("Starting JSON-RPC MCP server with SSE transport on port {}", port);

        // For now, fall back to stdio until SSE transport is properly configured
        info!("SSE transport not yet properly configured, falling back to stdio");
        self.start_stdio().await
    }

    /// Get server information
    pub fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "anytype-mcp-server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some(format!(
                "This server provides {} tools converted from an OpenAPI specification. Each tool corresponds to an API endpoint that can be called.",
                self.tools.len()
            )),
        }
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
        _request: Option<PaginatedRequestParamInner>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools: Vec<Tool> = self.tools.iter().map(|mcp_tool| {
            Tool {
                name: mcp_tool.name.clone().into(),
                description: mcp_tool.description.clone().map(|d| d.into()).unwrap_or_else(|| "".into()),
                input_schema: Arc::new(Self::convert_schema_to_tool_input(&mcp_tool.input_schema).as_object().unwrap().clone()),
            }
        }).collect();

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        CallToolRequestParam { name, arguments }: CallToolRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        info!("Calling tool: {}", name);

        // Find the tool - convert name to string for lookup
        let tool = self.tool_map.get(name.as_ref())
            .ok_or_else(|| McpError::invalid_params("Tool not found", None))?;

        // Execute the tool using the HTTP client
        let args = arguments.unwrap_or_else(|| Map::new());
        let args_value = Value::Object(args);

        match self.http_client.execute_tool(tool, args_value).await {
            Ok(result) => {
                info!("Tool '{}' executed successfully", name);
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
                )]))
            }
            Err(e) => {
                error!("Tool '{}' execution failed: {:?}", name, e);
                Ok(CallToolResult::error(vec![Content::text(
                    format!("Tool execution failed: {}", e)
                )]))
            }
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParamInner>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![],
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        _request: ReadResourceRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        Err(McpError::invalid_request("read_resource not supported", None))
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParamInner>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            prompts: vec![],
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        _request: GetPromptRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        Err(McpError::invalid_request("get_prompt not supported", None))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParamInner>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
        })
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        info!("Initializing MCP server");
        Ok(self.get_info())
    }
}
