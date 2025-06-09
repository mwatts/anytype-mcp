use anyhow::Result;
use figment::{Figment, providers::{Format, Json, Toml, Env}};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub spec_path: Option<String>,
    pub base_url: Option<String>,
    pub headers: HashMap<String, String>,
    pub timeout_seconds: Option<u64>,
    pub max_retries: Option<u32>,
    /// API key for authentication. Can be set via configuration or the ANYTYPE_API_KEY environment variable.
    /// When provided, this will be used to add an "Authorization: Bearer <api_key>" header to all requests.
    pub api_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            spec_path: None,
            base_url: None,
            headers: HashMap::new(),
            timeout_seconds: Some(30),
            max_retries: Some(3),
            api_key: None,
        }
    }
}

impl Config {
    /// Load configuration from files and environment variables.
    ///
    /// Configuration is loaded in the following priority order:
    /// 1. Configuration files: anytype-mcp.toml, anytype-mcp.json
    /// 2. Environment variables with ANYTYPE_MCP_ prefix
    /// 3. Special environment variables:
    ///    - `ANYTYPE_API_KEY`: Authentication token for Anytype API
    ///    - `OPENAPI_MCP_HEADERS`: Legacy JSON format for additional headers
    ///
    /// The ANYTYPE_API_KEY environment variable is used to automatically set the Authorization header
    /// as "Bearer <ANYTYPE_API_KEY>" for all outgoing HTTP requests.
    pub fn load(spec_path: Option<&str>) -> Result<Self> {
        let mut config: Config = Figment::new()
            .merge(Toml::file("anytype-mcp.toml"))
            .merge(Json::file("anytype-mcp.json"))
            .merge(Env::prefixed("ANYTYPE_MCP_"))
            .extract()
            .unwrap_or_default();

        // Handle legacy environment variable format
        if let Ok(headers_json) = env::var("OPENAPI_MCP_HEADERS") {
            if let Ok(headers) = serde_json::from_str::<HashMap<String, String>>(&headers_json) {
                config.headers.extend(headers);
            }
        }

        // Read API key from environment variable
        if config.api_key.is_none() {
            config.api_key = env::var("ANYTYPE_API_KEY").ok();
        }

        // Override spec path if provided
        if let Some(path) = spec_path {
            config.spec_path = Some(path.to_string());
        }

        Ok(config)
    }

    #[allow(dead_code)]
    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }

    #[allow(dead_code)]
    pub fn get_authorization_header(&self) -> Option<&String> {
        self.get_header("Authorization")
    }

    /// Get the API key, either from config or environment variable
    #[allow(dead_code)]
    pub fn get_api_key(&self) -> Option<&String> {
        self.api_key.as_ref()
    }
}
