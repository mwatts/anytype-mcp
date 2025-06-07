#[cfg(test)]
mod integration_tests {
    use crate::server::mcp_server::AnytypeMcpServer;
    use crate::config::Config;
    use crate::utils::AnytypeMcpError;
    use serde_json::json;
    use tempfile::NamedTempFile;
    use std::io::Write;
    use tokio::runtime::Runtime;

    fn create_comprehensive_openapi_spec() -> String {
        json!({
            "openapi": "3.0.0",
            "info": {
                "title": "Comprehensive Test API",
                "version": "1.0.0"
            },
            "servers": [
                {
                    "url": "https://api.test.com"
                }
            ],
            "paths": {
                "/users": {
                    "get": {
                        "operationId": "getUsers",
                        "summary": "Get all users",
                        "parameters": [
                            {
                                "name": "limit",
                                "in": "query",
                                "required": false,
                                "schema": {
                                    "type": "integer",
                                    "minimum": 1,
                                    "maximum": 100
                                }
                            },
                            {
                                "name": "filter",
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
                    },
                    "post": {
                        "operationId": "createUser",
                        "summary": "Create a new user",
                        "requestBody": {
                            "required": true,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "name": {
                                                "type": "string"
                                            },
                                            "email": {
                                                "type": "string",
                                                "format": "email"
                                            },
                                            "age": {
                                                "type": "integer",
                                                "minimum": 0
                                            }
                                        },
                                        "required": ["name", "email"]
                                    }
                                }
                            }
                        },
                        "responses": {
                            "201": {
                                "description": "Created"
                            }
                        }
                    }
                },
                "/users/{id}": {
                    "get": {
                        "operationId": "getUserById",
                        "summary": "Get user by ID",
                        "parameters": [
                            {
                                "name": "id",
                                "in": "path",
                                "required": true,
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

    async fn create_comprehensive_test_server() -> AnytypeMcpServer {
        let spec_content = create_comprehensive_openapi_spec();
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(spec_content.as_bytes()).unwrap();
        let spec_path = temp_file.path().to_string_lossy().to_string();

        let config = Config::default();
        AnytypeMcpServer::new(Some(spec_path), config).await.unwrap()
    }

    #[test]
    fn test_comprehensive_server_creation() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = create_comprehensive_test_server().await;

            // Verify all tools were created
            assert_eq!(server.get_tools().len(), 3);

            let tool_names: Vec<&str> = server.get_tools().iter().map(|t| t.name.as_str()).collect();
            assert!(tool_names.contains(&"getUsers"));
            assert!(tool_names.contains(&"createUser"));
            assert!(tool_names.contains(&"getUserById"));

            // Verify tool details
            let get_users_tool = server.get_tool("getUsers").unwrap();
            assert_eq!(get_users_tool.method, "GET");
            assert_eq!(get_users_tool.path, "/users");

            let create_user_tool = server.get_tool("createUser").unwrap();
            assert_eq!(create_user_tool.method, "POST");
            assert_eq!(create_user_tool.path, "/users");

            let get_user_tool = server.get_tool("getUserById").unwrap();
            assert_eq!(get_user_tool.method, "GET");
            assert_eq!(get_user_tool.path, "/users/{id}");
        });
    }

    #[test]
    fn test_complex_schema_conversion() {
        let complex_schema = json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "minLength": 1
                        },
                        "email": {
                            "type": "string",
                            "format": "email"
                        },
                        "preferences": {
                            "type": "object",
                            "properties": {
                                "theme": {
                                    "type": "string",
                                    "enum": ["light", "dark"]
                                },
                                "notifications": {
                                    "type": "boolean"
                                }
                            }
                        },
                        "tags": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        }
                    },
                    "required": ["name", "email"]
                },
                "metadata": {
                    "type": "object",
                    "additionalProperties": true
                }
            },
            "required": ["user"]
        });

        let converted = AnytypeMcpServer::convert_schema_to_tool_input(&complex_schema);

        // Should handle complex nested schemas
        assert_eq!(converted.r#type, "object");
        assert!(converted.properties.is_some());
        assert!(converted.required.is_some());
    }

    #[test]
    fn test_shared_runtime_performance() {
        // Test that creating new runtimes doesn't create performance bottlenecks
        let start = std::time::Instant::now();

        for _i in 0..100 {
            let runtime = Runtime::new().expect("Failed to create runtime");

            let _result = runtime.block_on(async {
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                "test"
            });
        }

        let duration = start.elapsed();

        // Should complete in reasonable time (less than 5 seconds for 100 iterations)
        assert!(duration < std::time::Duration::from_secs(5));
    }

    #[test]
    fn test_error_propagation() {
        // Test that errors are properly converted and propagated
        let invalid_config = Config::default();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            AnytypeMcpServer::new(Some("nonexistent.json".to_string()), invalid_config).await
        });

        assert!(result.is_err());
        // Should be an IO error for missing file
        if let Err(error) = result {
            assert!(matches!(error, AnytypeMcpError::Io(_)));
        }
    }

    #[test]
    fn test_tool_schema_edge_cases() {
        // Test edge cases for schema conversion
        let edge_cases = vec![
            // Empty schema
            json!({}),
            // Schema with only type
            json!({"type": "string"}),
            // Schema with null properties
            json!({"type": "object", "properties": null}),
            // Schema with empty properties
            json!({"type": "object", "properties": {}}),
            // Schema with no type
            json!({"properties": {"test": {"type": "string"}}}),
        ];

        for schema in edge_cases {
            let converted = AnytypeMcpServer::convert_schema_to_tool_input(&schema);
            // Should not panic and should return a valid schema
            assert!(!converted.r#type.is_empty());
        }
    }
}
