pub mod mcp_server;

pub use mcp_server::AnytypeMcpServer;

#[cfg(test)]
mod comprehensive_tests;

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio;

    use crate::server::AnytypeMcpServer;
    use crate::config::Config;
    use crate::openapi::McpTool;

    #[tokio::test]
    async fn test_tool_input_schema_conversion() {
        // Test simple object schema
        let simple_schema = json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results"
                }
            },
            "required": ["query"]
        });

        let converted = AnytypeMcpServer::convert_schema_to_tool_input(&simple_schema);
        
        assert_eq!(converted.r#type, "object");
        assert!(converted.properties.is_some());
        assert!(converted.required.is_some());
        
        let properties = converted.properties.unwrap();
        assert!(properties.contains_key("query"));
        assert!(properties.contains_key("limit"));
        
        let required = converted.required.unwrap();
        assert_eq!(required, vec!["query"]);
    }

    #[tokio::test]
    async fn test_schema_conversion_with_nested_objects() {
        let nested_schema = json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "email": {"type": "string"}
                    },
                    "required": ["name"]
                },
                "metadata": {
                    "type": "object",
                    "properties": {
                        "tags": {
                            "type": "array",
                            "items": {"type": "string"}
                        }
                    }
                }
            },
            "required": ["user"]
        });

        let converted = AnytypeMcpServer::convert_schema_to_tool_input(&nested_schema);
        
        assert_eq!(converted.r#type, "object");
        assert!(converted.properties.is_some());
        
        let properties = converted.properties.unwrap();
        assert!(properties.contains_key("user"));
        assert!(properties.contains_key("metadata"));
        
        let required = converted.required.unwrap();
        assert_eq!(required, vec!["user"]);
    }

    #[tokio::test]
    async fn test_empty_schema_conversion() {
        let empty_schema = json!({});
        let converted = AnytypeMcpServer::convert_schema_to_tool_input(&empty_schema);
        
        // Should default to object type with no properties or required fields
        assert_eq!(converted.r#type, "object");
        assert!(converted.properties.is_none());
        assert!(converted.required.is_none());
    }

    #[tokio::test]
    async fn test_array_schema_conversion() {
        let array_schema = json!({
            "type": "array",
            "items": {
                "type": "string"
            }
        });

        let converted = AnytypeMcpServer::convert_schema_to_tool_input(&array_schema);
        assert_eq!(converted.r#type, "array");
    }

    #[tokio::test] 
    async fn test_server_creation_with_minimal_config() {
        let config = Config::default();
        
        // Create a simple test OpenAPI spec file
        let test_spec = json!({
            "openapi": "3.0.0",
            "info": {
                "title": "Test API",
                "version": "1.0.0"
            },
            "paths": {
                "/test": {
                    "get": {
                        "operationId": "test_operation",
                        "summary": "Test operation",
                        "responses": {
                            "200": {
                                "description": "Success"
                            }
                        }
                    }
                }
            }
        });

        // Write test spec to a temporary file
        let temp_dir = tempfile::tempdir().unwrap();
        let spec_path = temp_dir.path().join("test_spec.json");
        tokio::fs::write(&spec_path, serde_json::to_string_pretty(&test_spec).unwrap())
            .await
            .unwrap();

        // Test server creation
        let result = AnytypeMcpServer::new(
            Some(spec_path.to_string_lossy().to_string()),
            config
        ).await;

        assert!(result.is_ok());
        let server = result.unwrap();
        assert_eq!(server.get_tools().len(), 1);
        assert_eq!(server.get_tools()[0].name, "test_operation");
    }

    #[tokio::test]
    async fn test_mcp_tool_clone() {
        let tool = McpTool {
            name: "test_tool".to_string(),
            description: Some("Test description".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                }
            }),
            method: "GET".to_string(),
            path: "/test".to_string(),
            operation_id: "test_op".to_string(),
        };

        // Test that the tool is cloneable
        let cloned_tool = tool.clone();
        assert_eq!(tool.name, cloned_tool.name);
        assert_eq!(tool.description, cloned_tool.description);
        assert_eq!(tool.method, cloned_tool.method);
        assert_eq!(tool.path, cloned_tool.path);
        assert_eq!(tool.operation_id, cloned_tool.operation_id);
    }

    #[cfg(test)]
    mod integration_tests {
        use super::*;
        use mockito::Server as MockServer;

        #[tokio::test]
        async fn test_tool_execution_with_mock_server() {
            // Start mock server
            let mut mock_server = MockServer::new_async().await;
            
            // Create mock endpoint - NOTE: Mock server matches exact paths, not including base URL
            let mock = mock_server
                .mock("GET", "/api/test?query=test_value")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{"status": "success", "data": "test_response"}"#)
                .create_async()
                .await;

            // Create config with mock server URL
            let mut config = Config::default();
            config.base_url = Some(mock_server.url());

            // Create test OpenAPI spec
            let test_spec = json!({
                "openapi": "3.0.0",
                "info": {
                    "title": "Test API",
                    "version": "1.0.0"
                },
                "paths": {
                    "/api/test": {
                        "get": {
                            "operationId": "test_api_call",
                            "summary": "Test API call",
                            "parameters": [{
                                "name": "query",
                                "in": "query",
                                "required": true,
                                "schema": {
                                    "type": "string"
                                }
                            }],
                            "responses": {
                                "200": {
                                    "description": "Success"
                                }
                            }
                        }
                    }
                }
            });

            // Write test spec to a temporary file
            let temp_dir = tempfile::tempdir().unwrap();
            let spec_path = temp_dir.path().join("test_spec.json");
            tokio::fs::write(&spec_path, serde_json::to_string_pretty(&test_spec).unwrap())
                .await
                .unwrap();

            // Create server
            let server = AnytypeMcpServer::new(
                Some(spec_path.to_string_lossy().to_string()),
                config
            ).await.unwrap();

            // Verify tool was created
            assert_eq!(server.get_tools().len(), 1);
            let tool = &server.get_tools()[0];
            assert_eq!(tool.name, "test_api_call");

            // Test HTTP client execution directly
            let params = json!({"query": "test_value"});
            let result = server.http_client.execute_tool(tool, params).await;

            match &result {
                Ok(response) => {
                    assert_eq!(response["status"], "success");
                    assert_eq!(response["data"], "test_response");
                }
                Err(e) => {
                    println!("Tool execution failed: {:?}", e);
                    println!("Tool path: {}", tool.path);
                    println!("Tool method: {}", tool.method);
                    println!("Mock server URL: {}", mock_server.url());
                    panic!("Tool execution failed: {:?}", e);
                }
            }

            // Verify mock was called
            mock.assert_async().await;
        }
    }
}
