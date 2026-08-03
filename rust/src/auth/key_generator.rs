use std::io::{self, Write};

use serde_json::{Value, json};
use tracing::info;

use crate::client::http_client::ANYTYPE_API_VERSION;
use crate::config::Config;
use crate::openapi::{get_base_url, load_openapi_spec};
use crate::utils::{AnytypeMcpError, Result as McpResult};

/// App name shown in the Anytype pairing dialog; matches the TS implementation.
const APP_NAME: &str = "anytype_mcp_server";

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
                spec_path
                    .rsplit_once('/')
                    .map(|s| s.0.to_string())
                    .unwrap_or_else(|| crate::config::DEFAULT_BASE_URL.to_string())
            } else {
                // Load local spec to get base URL
                let spec = load_openapi_spec(&spec_path).await?;
                get_base_url(&spec).unwrap_or_else(|| crate::config::DEFAULT_BASE_URL.to_string())
            }
        } else {
            config
                .base_url
                .clone()
                .unwrap_or_else(|| crate::config::DEFAULT_BASE_URL.to_string())
        };

        Ok(Self { base_url, config })
    }

    /// Run the interactive challenge flow: request a challenge, ask the user
    /// for the 4-digit code shown in Anytype, and exchange it for an API key.
    pub async fn generate_interactive(&self) -> McpResult<()> {
        info!("Starting API key generation against {}", self.base_url);
        println!("Requesting authentication challenge from {}", self.base_url);

        let client = reqwest::Client::new();
        let challenge_id = self.start_challenge(&client).await?;

        println!("A 4-digit code should now be displayed in your Anytype app.");
        let code = Self::prompt("Enter the 4-digit code: ")?;

        let (api_key, api_version) = self
            .complete_challenge(&client, &challenge_id, &code)
            .await?;

        println!("\n✅ Your API key: {}", api_key);
        println!("\nAdd this to your MCP client configuration:\n");
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "mcpServers": {
                    "anytype": {
                        "command": "anytype-mcp",
                        "env": {
                            "ANYTYPE_API_KEY": api_key,
                        }
                    }
                }
            }))
            .expect("static JSON structure serializes")
        );
        println!("\n(Anytype-Version: {})", api_version);

        Ok(())
    }

    /// `POST /v1/auth/challenges` — makes Anytype display a pairing code.
    async fn start_challenge(&self, client: &reqwest::Client) -> McpResult<String> {
        let response = client
            .post(format!("{}/v1/auth/challenges", self.base_url))
            .header("Anytype-Version", ANYTYPE_API_VERSION)
            .json(&json!({ "app_name": APP_NAME }))
            .send()
            .await
            .map_err(|e| {
                AnytypeMcpError::Auth(format!(
                    "Failed to start authentication: {}. Please ensure Anytype is running and reachable at {}.",
                    e, self.base_url
                ))
            })?;

        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|e| AnytypeMcpError::Auth(format!("Invalid challenge response: {}", e)))?;

        body.get("challenge_id")
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or_else(|| {
                AnytypeMcpError::Auth(format!(
                    "Failed to get challenge ID (HTTP {}): {}",
                    status, body
                ))
            })
    }

    /// `POST /v1/auth/api_keys` — exchanges the challenge + code for an API
    /// key. Returns the key and the server's reported Anytype-Version.
    async fn complete_challenge(
        &self,
        client: &reqwest::Client,
        challenge_id: &str,
        code: &str,
    ) -> McpResult<(String, String)> {
        let response = client
            .post(format!("{}/v1/auth/api_keys", self.base_url))
            .header("Anytype-Version", ANYTYPE_API_VERSION)
            .json(&json!({ "challenge_id": challenge_id, "code": code }))
            .send()
            .await
            .map_err(|e| {
                AnytypeMcpError::Auth(format!("Failed to complete authentication: {}", e))
            })?;

        let status = response.status();
        let api_version = response
            .headers()
            .get("anytype-version")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(ANYTYPE_API_VERSION)
            .to_string();
        let body: Value = response
            .json()
            .await
            .map_err(|e| AnytypeMcpError::Auth(format!("Invalid API key response: {}", e)))?;

        let api_key = body
            .get("api_key")
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or_else(|| {
                AnytypeMcpError::Auth(format!(
                    "Failed to complete authentication (HTTP {}): {}",
                    status, body
                ))
            })?;

        Ok((api_key, api_version))
    }

    fn prompt(message: &str) -> McpResult<String> {
        print!("{}", message);
        io::stdout().flush().map_err(AnytypeMcpError::Io)?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(AnytypeMcpError::Io)?;
        Ok(input.trim().to_string())
    }
}
