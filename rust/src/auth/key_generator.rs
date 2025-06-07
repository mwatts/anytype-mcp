use std::io::{self, Write};
use tracing::info;

use crate::config::Config;
use crate::openapi::{load_openapi_spec, get_base_url};
use crate::utils::{AnytypeMcpError, Result as McpResult};

const AUTH_TEMPLATE: &str = r#"
# Anytype API Key Setup

To use the Anytype MCP Server, you need to configure your API key.

## Option 1: Get API Key from Anytype App

1. Open Anytype
2. Go to Settings
3. Navigate to API Keys
4. Create a new API key
5. Copy the API key

## Option 2: Use this CLI tool

This tool can help you generate an API key interactively.

Base URL: {{base_url}}

Please visit the Anytype application to generate your API key.

## Configuration

Add the following to your MCP client configuration:

```json
{
  "mcpServers": {
    "anytype": {
      "command": "anytype-mcp",
      "env": {
        "OPENAPI_MCP_HEADERS": "{\"Authorization\":\"Bearer YOUR_API_KEY_HERE\", \"Anytype-Version\":\"2025-05-20\"}"
      }
    }
  }
}
```

Or set environment variables:
```bash
export ANYTYPE_MCP_HEADERS='{"Authorization":"Bearer YOUR_API_KEY_HERE", "Anytype-Version":"2025-05-20"}'
```
"#;

pub struct KeyGenerator {
    base_url: String,
    #[allow(dead_code)]
    config: Config,
}

impl KeyGenerator {
    pub async fn new(spec_path: Option<String>, config: Config) -> McpResult<Self> {
        let base_url = if let Some(spec_path) = spec_path {
            if spec_path.starts_with("http") {
                // For remote specs, try to extract base URL from the URL
                spec_path.rsplit_once('/').map(|s| s.0.to_string())
                    .unwrap_or_else(|| "http://localhost:31009".to_string())
            } else {
                // Load local spec to get base URL
                let spec = load_openapi_spec(&spec_path).await?;
                get_base_url(&spec).unwrap_or_else(|| "http://localhost:31009".to_string())
            }
        } else {
            config.base_url.clone()
                .unwrap_or_else(|| "http://localhost:31009".to_string())
        };

        Ok(Self { base_url, config })
    }

    pub async fn generate_interactive(&self) -> McpResult<()> {
        info!("Starting interactive API key generation");

        // Simple string replacement instead of template engine
        let output = AUTH_TEMPLATE.replace("{{base_url}}", &self.base_url);

        println!("{}", output);

        // Interactive prompts
        self.prompt_for_key().await?;

        Ok(())
    }

    async fn prompt_for_key(&self) -> McpResult<()> {
        print!("\nWould you like to test a connection with your API key? (y/N): ");
        io::stdout().flush().map_err(AnytypeMcpError::Io)?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(AnytypeMcpError::Io)?;

        if input.trim().to_lowercase() == "y" {
            self.test_connection().await?;
        }

        Ok(())
    }

    async fn test_connection(&self) -> McpResult<()> {
        print!("Enter your API key: ");
        io::stdout().flush().map_err(AnytypeMcpError::Io)?;

        let mut api_key = String::new();
        io::stdin().read_line(&mut api_key).map_err(AnytypeMcpError::Io)?;
        let api_key = api_key.trim();

        if api_key.is_empty() {
            println!("No API key provided.");
            return Ok(());
        }

        info!("Testing connection with provided API key");

        // Test the connection
        let client = reqwest::Client::new();
        let response = client
            .get(&format!("{}/health", self.base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Anytype-Version", "2025-05-20")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    println!("✅ Connection successful!");
                    println!("Your API key is working correctly.");
                } else {
                    println!("❌ Connection failed with status: {}", resp.status());
                    if let Ok(text) = resp.text().await {
                        println!("Error: {}", text);
                    }
                }
            }
            Err(e) => {
                println!("❌ Connection failed: {}", e);
                println!("Please check if the Anytype service is running and accessible.");
            }
        }

        Ok(())
    }
}
