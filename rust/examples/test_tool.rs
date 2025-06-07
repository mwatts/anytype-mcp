use mcpr::{Tool, schema::ToolInputSchema};
use serde_json::json;

fn main() {
    // Test what fields Tool expects
    let schema = ToolInputSchema::JsonSchema(json!({}));
    let tool = Tool {
        name: "test".to_string(),
        description: Some("test".to_string()),
        input_schema: schema,
    };
    println!("{:?}", tool);
}
