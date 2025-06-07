use mcpr::{Tool, schema::ToolInputSchema};
use serde_json::json;
use std::collections::HashMap;

fn main() {
    println!("Testing MCPR ToolInputSchema structure...");

    // Create a ToolInputSchema using the correct structure
    let schema = ToolInputSchema {
        r#type: "object".to_string(),
        properties: Some([
            ("param1".to_string(), json!({
                "type": "string",
                "description": "A test parameter"
            })),
            ("param2".to_string(), json!({
                "type": "number",
                "description": "A numeric parameter"
            }))
        ].into_iter().collect()),
        required: Some(vec!["param1".to_string()]),
    };

    println!("✅ Successfully created ToolInputSchema: {:?}", schema);

    // Create a Tool using this schema
    let tool = Tool {
        name: "test_tool".to_string(),
        description: Some("A test tool".to_string()),
        input_schema: schema,
    };

    println!("✅ Successfully created Tool: {:?}", tool);
}
