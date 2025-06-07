// Test the ToolInputSchema structure
use mcpr::schema::ToolInputSchema;
use serde_json::json;

fn main() {
    println!("Testing ToolInputSchema variants...");

    // Try to create different variants to see what's available
    let schema = json!({
        "type": "object",
        "properties": {
            "param1": {"type": "string"}
        }
    });

    // Just print the type to understand the structure
    println!("ToolInputSchema: {:?}", std::any::type_name::<ToolInputSchema>());
}
