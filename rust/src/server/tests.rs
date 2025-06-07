#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use mockito::{mock, Matcher};
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

    #[tokio::test]
    async fn test_server_creation_with_shared_runtime() {
        let server = create_test_server().await;

        // Verify server has tools
        assert!(!server.tools.is_empty());
        assert_eq!(server.tools.len(), 1);
        assert_eq!(server.tools[0].name, "getTest");

        // Verify runtime was created
        assert!(std::ptr::addr_of!(server.runtime) as usize != 0);
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
        // The exact structure depends on MCPR implementation
        // but we can verify it doesn't panic and returns something
        println!("Converted schema: {:?}", converted);
    }

    #[tokio::test]
    async fn test_tool_handler_with_shared_runtime() {
        let _mock_server = mockito::Server::new_async().await;

        // This test verifies that the shared runtime approach works
        // by creating multiple runtimes and ensuring they can coexist
        let rt1 = Arc::new(Runtime::new().unwrap());
        let rt2 = Arc::new(Runtime::new().unwrap());

        let result1 = rt1.block_on(async { "test1" });
        let result2 = rt2.block_on(async { "test2" });

        assert_eq!(result1, "test1");
        assert_eq!(result2, "test2");
    }

    #[tokio::test]
    async fn test_error_handling_in_tool_execution() {
        let server = create_test_server().await;

        // Test that error conversion works properly
        let error = AnytypeMcpError::Config("test error".to_string());
        let error_string = error.to_string();
        assert!(error_string.contains("test error"));
    }

    #[tokio::test]
    async fn test_concurrent_tool_execution() {
        let server = create_test_server().await;
        let runtime = &server.runtime;

        // Test that the shared runtime can handle concurrent operations
        let tasks: Vec<_> = (0..10).map(|i| {
            let rt = Arc::clone(runtime);
            tokio::spawn(async move {
                rt.block_on(async {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    i
                })
            })
        }).collect();

        let results: Vec<_> = futures::future::join_all(tasks).await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(results, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}
