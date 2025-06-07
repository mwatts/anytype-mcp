use std::collections::HashMap;
use std::sync::OnceLock;

use mcpr::{
    transport::stdio::StdioTransport,
    server::{Server, ServerConfig},
    Tool, schema::ToolInputSchema,
};
use serde_json::Value;
use tokio::runtime::Runtime;
use tracing::{info, debug, error};

use crate::config::Config;
use crate::client::HttpClient;
use crate::openapi::{load_openapi_spec, get_base_url, OpenApiParser, McpTool};
use crate::utils::{AnytypeMcpError, Result as McpResult};

// Global shared runtime for MCPR tool handlers
static SHARED_RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub struct AnytypeMcpServer {
    #[allow(dead_code)]
    config: Config,
    pub(crate) http_client: HttpClient,
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
        info!("Starting MCP server with {} tools", self.tools.len());

        // Create server configuration
        let mut server_config = ServerConfig::new()
            .with_name("Anytype MCP Server")
            .with_version("1.0.0");

        // Convert tools to MCPR format and add them to the server config
        for tool in &self.tools {
            let mcpr_tool = Tool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                input_schema: Self::convert_schema_to_tool_input(&tool.input_schema),
            };

            debug!("Registering tool: {} with schema: {:?}", tool.name, mcpr_tool.input_schema);
            server_config = server_config.with_tool(mcpr_tool);
        }

        info!("Creating MCPR server with {} tools registered", self.tools.len());

        // Create server
        let mut server = Server::new(server_config);

        info!("MCPR server created successfully");

        // Register tool handlers with actual HTTP client execution
        for tool in &self.tools {
            let tool_name = tool.name.clone();
            let tool_clone = tool.clone();
            let client_clone = self.http_client.clone();

            server.register_tool_handler(&tool_name, move |params: Value| {
                debug!("Executing tool: {} with params: {:?}", tool_clone.name, params);

                // Get or create the shared runtime
                let runtime = SHARED_RUNTIME.get_or_init(|| {
                    Runtime::new().expect("Failed to create shared runtime")
                });

                // Use the shared runtime for async execution within the sync handler
                let result = runtime.block_on(async {
                    client_clone.execute_tool(&tool_clone, params).await
                });

                match result {
                    Ok(response) => {
                        debug!("Tool {} executed successfully", tool_clone.name);
                        Ok(response)
                    }
                    Err(e) => {
                        error!("Tool {} execution failed: {}", tool_clone.name, e);
                        Err(mcpr::error::MCPError::Protocol(format!("Tool execution failed: {}", e)))
                    }
                }
            }).map_err(|e| AnytypeMcpError::McpProtocol(e.to_string()))?;
        }

        info!("Registered {} tool handlers with actual HTTP execution", self.tools.len());

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

    /// Convert a serde_json::Value schema to MCPR ToolInputSchema
    pub fn convert_schema_to_tool_input(schema: &Value) -> ToolInputSchema {
        let mut tool_schema = ToolInputSchema {
            r#type: "object".to_string(),
            properties: None,
            required: None,
        };

        if let Some(obj) = schema.as_object() {
            // Extract type
            if let Some(schema_type) = obj.get("type").and_then(|v| v.as_str()) {
                tool_schema.r#type = schema_type.to_string();
            }

            // Extract properties
            if let Some(properties) = obj.get("properties").and_then(|v| v.as_object()) {
                let mut props_map = HashMap::new();
                for (key, value) in properties {
                    props_map.insert(key.clone(), value.clone());
                }
                tool_schema.properties = Some(props_map);
            }

            // Extract required fields
            if let Some(required) = obj.get("required").and_then(|v| v.as_array()) {
                let required_fields: Vec<String> = required
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                if !required_fields.is_empty() {
                    tool_schema.required = Some(required_fields);
                }
            }
        }

        tool_schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serde_json::json;
    use tempfile::NamedTempFile;
    use std::io::Write;

    fn create_test_openapi_spec() -> String {
        json!({
            "openapi": "3.0.0",
            "info": {
                "title": "Test API",
                "version": "1.0.0"
            },
            "servers": [
                {
                    "url": "https://api.test.com"
                }
            ],
            "paths": {
                "/test": {
                    "get": {
                        "operationId": "getTest",
                        "summary": "Get test data",
                        "parameters": [
                            {
                                "name": "query",
                                "in": "query",
                                "required": false,
                                "schema": {
                                    "type": "string"
                                }
                            }
                        ],
                        "responses": {
                            "200": {
                                "description": "Success"
                            }
                        }
                    }
                }
            }
        }).to_string()
    }

    async fn create_test_server() -> AnytypeMcpServer {
        let spec_content = create_test_openapi_spec();
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(spec_content.as_bytes()).unwrap();
        let spec_path = temp_file.path().to_string_lossy().to_string();

        let config = Config::default();
        AnytypeMcpServer::new(Some(spec_path), config).await.unwrap()
    }

    #[test]
    fn test_server_creation_with_shared_runtime() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = create_test_server().await;

            // Verify server has tools
            assert!(!server.tools.is_empty());
            assert_eq!(server.tools.len(), 1);
            assert_eq!(server.tools[0].name, "getTest");

            // Verify the shared runtime system can be accessed
            // (it might already be initialized by other tests)
            let _runtime = SHARED_RUNTIME.get_or_init(|| {
                Runtime::new().expect("Failed to create runtime")
            });
        });
    }

    #[test]
    fn test_schema_conversion() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                }
            }
        });

        let converted = AnytypeMcpServer::convert_schema_to_tool_input(&schema);

        // Should convert to a valid ToolInputSchema
        assert_eq!(converted.r#type, "object");
    }

    #[test]
    fn test_shared_runtime_creation() {
        // Test that the shared runtime system works
        let runtime = SHARED_RUNTIME.get_or_init(|| {
            Runtime::new().expect("Failed to create runtime")
        });

        let result1 = runtime.block_on(async { "test1" });
        let result2 = runtime.block_on(async { "test2" });

        assert_eq!(result1, "test1");
        assert_eq!(result2, "test2");

        // Verify the same runtime is reused
        let runtime2 = SHARED_RUNTIME.get().unwrap();
        assert!(std::ptr::eq(runtime, runtime2));
    }

    #[test]
    fn test_error_handling() {
        // Test that error conversion works properly
        let error = AnytypeMcpError::Config("test error".to_string());
        let error_string = error.to_string();
        assert!(error_string.contains("test error"));
    }

    #[test]
    fn test_tool_lookup() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = create_test_server().await;

            // Test tool lookup functionality
            let tool = server.get_tool("getTest");
            assert!(tool.is_some());
            assert_eq!(tool.unwrap().name, "getTest");

            let missing_tool = server.get_tool("nonexistent");
            assert!(missing_tool.is_none());
        });
    }
}
