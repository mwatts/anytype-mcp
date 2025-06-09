pub mod hybrid_server;
pub mod json_rpc_server;

// Use the new JSON-RPC server as the default
pub use hybrid_server::{HybridMcpServer, ServerMode};
pub use json_rpc_server::AnytypeJsonRpcServer;

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio;

    use crate::server::AnytypeJsonRpcServer;

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

        let converted = AnytypeJsonRpcServer::convert_schema_to_tool_input(&simple_schema);

        assert_eq!(converted["type"], "object");
        assert!(converted["properties"].is_object());
        assert!(converted["required"].is_array());

        let properties = converted["properties"].as_object().unwrap();
        assert!(properties.contains_key("query"));
        assert!(properties.contains_key("limit"));

        let required = converted["required"].as_array().unwrap();
        assert_eq!(required, &vec![json!("query")]);
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

        let converted = AnytypeJsonRpcServer::convert_schema_to_tool_input(&nested_schema);

        assert_eq!(converted["type"], "object");
        assert!(converted["properties"].is_object());

        let properties = converted["properties"].as_object().unwrap();
        assert!(properties.contains_key("user"));
        assert!(properties.contains_key("metadata"));

        let required = converted["required"].as_array().unwrap();
        assert_eq!(required, &vec![json!("user")]);
    }

    #[tokio::test]
    async fn test_empty_schema_conversion() {
        let empty_schema = json!({});
        let converted = AnytypeJsonRpcServer::convert_schema_to_tool_input(&empty_schema);

        // Should default to object type with no properties or required fields
        assert_eq!(converted["type"], "object");
        assert!(converted.get("properties").is_none() || converted["properties"].is_null());
        assert!(converted.get("required").is_none() || converted["required"].is_null());
    }

    #[tokio::test]
    async fn test_array_schema_conversion() {
        let array_schema = json!({
            "type": "array",
            "items": {
                "type": "string"
            }
        });

        let converted = AnytypeJsonRpcServer::convert_schema_to_tool_input(&array_schema);
        assert_eq!(converted["type"], "array");
    }
}