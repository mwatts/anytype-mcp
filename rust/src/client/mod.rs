pub mod http_client;

pub use http_client::HttpClient;

#[cfg(test)]
mod tests {
    use tokio;
    use serde_json::json;
    use crate::client::HttpClient;
    use crate::config::Config;
    use crate::openapi::McpTool;

    #[tokio::test]
    async fn test_http_client_simple() {
        let config = Config::default();
        let client = HttpClient::new(&config, "https://httpbin.org".to_string()).unwrap();

        let tool = McpTool {
            name: "test_tool".to_string(),
            description: Some("Test tool".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                }
            }),
            method: "GET".to_string(),
            path: "/get".to_string(),
            operation_id: "test_get".to_string(),
        };

        let params = json!({"param": "test_value"});
        let result = client.execute_tool(&tool, params).await;

        // This should succeed with httpbin.org
        assert!(result.is_ok(), "HTTP client execution failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_schema_conversion_edge_cases() {
        use crate::server::AnytypeMcpServer;

        // Test with missing properties
        let minimal_schema = json!({"type": "object"});
        let converted = AnytypeMcpServer::convert_schema_to_tool_input(&minimal_schema);
        assert_eq!(converted.r#type, "object");
        assert!(converted.properties.is_none());
        assert!(converted.required.is_none());

        // Test with string type
        let string_schema = json!({"type": "string"});
        let converted = AnytypeMcpServer::convert_schema_to_tool_input(&string_schema);
        assert_eq!(converted.r#type, "string");

        // Test with complex schema including additional properties
        let complex_schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "User name"},
                "age": {"type": "integer", "minimum": 0, "maximum": 150},
                "email": {"type": "string", "format": "email"}
            },
            "required": ["name", "email"],
            "additionalProperties": false
        });

        let converted = AnytypeMcpServer::convert_schema_to_tool_input(&complex_schema);
        assert_eq!(converted.r#type, "object");

        let properties = converted.properties.unwrap();
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("age"));
        assert!(properties.contains_key("email"));

        let required = converted.required.unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.contains(&"name".to_string()));
        assert!(required.contains(&"email".to_string()));
    }
}
