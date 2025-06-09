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
        use crate::server::AnytypeJsonRpcServer;

        // Test with missing properties
        let minimal_schema = json!({"type": "object"});
        let converted = AnytypeJsonRpcServer::convert_schema_to_tool_input(&minimal_schema);
        assert_eq!(converted["type"], "object");
        assert!(converted.get("properties").is_none() || converted["properties"].is_null());
        assert!(converted.get("required").is_none() || converted["required"].is_null());

        // Test with string type
        let string_schema = json!({"type": "string"});
        let converted = AnytypeJsonRpcServer::convert_schema_to_tool_input(&string_schema);
        assert_eq!(converted["type"], "string");

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

        let converted = AnytypeJsonRpcServer::convert_schema_to_tool_input(&complex_schema);
        assert_eq!(converted["type"], "object");

        let properties = converted["properties"].as_object().unwrap();
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("age"));
        assert!(properties.contains_key("email"));

        let required = converted["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.contains(&json!("name")));
        assert!(required.contains(&json!("email")));
    }

    #[tokio::test]
    async fn test_http_client_headers() {
        use std::collections::HashMap;

        // Test without API key
        let config = Config::default();
        let client = HttpClient::new(&config, "https://httpbin.org".to_string()).unwrap();

        let headers = client.get_default_headers();

        // Check that required headers are present
        assert!(headers.contains_key("Anytype-Version"));
        assert_eq!(headers.get("Anytype-Version").unwrap(), "2025-05-20");
        assert!(headers.contains_key("Content-Type"));
        assert_eq!(headers.get("Content-Type").unwrap(), "application/json");

        // Authorization header should not be present
        assert!(!headers.contains_key("Authorization"));

        // Test with API key
        let mut config_with_key = Config::default();
        config_with_key.api_key = Some("test_api_key_12345".to_string());
        let client_with_key = HttpClient::new(&config_with_key, "https://httpbin.org".to_string()).unwrap();

        let headers_with_key = client_with_key.get_default_headers();

        // Check that required headers are present
        assert!(headers_with_key.contains_key("Anytype-Version"));
        assert_eq!(headers_with_key.get("Anytype-Version").unwrap(), "2025-05-20");
        assert!(headers_with_key.contains_key("Content-Type"));
        assert_eq!(headers_with_key.get("Content-Type").unwrap(), "application/json");

        // Authorization header should be present with Bearer token
        assert!(headers_with_key.contains_key("Authorization"));
        assert_eq!(headers_with_key.get("Authorization").unwrap(), "Bearer test_api_key_12345");

        // Test with additional custom headers
        let mut custom_headers = HashMap::new();
        custom_headers.insert("Custom-Header".to_string(), "custom-value".to_string());
        let mut config_with_custom = Config::default();
        config_with_custom.headers = custom_headers;
        config_with_custom.api_key = Some("another_key".to_string());

        let client_with_custom = HttpClient::new(&config_with_custom, "https://httpbin.org".to_string()).unwrap();

        let headers_with_custom = client_with_custom.get_default_headers();

        // All headers should be present
        assert_eq!(headers_with_custom.get("Anytype-Version").unwrap(), "2025-05-20");
        assert_eq!(headers_with_custom.get("Content-Type").unwrap(), "application/json");
        assert_eq!(headers_with_custom.get("Authorization").unwrap(), "Bearer another_key");
        assert_eq!(headers_with_custom.get("Custom-Header").unwrap(), "custom-value");
    }
}
