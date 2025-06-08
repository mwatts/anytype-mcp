use anyhow::Result;
use figment::{Figment, providers::{Format, Json, Toml, Env}};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub spec_path: Option<String>,
    pub base_url: Option<String>,
    pub headers: HashMap<String, String>,
    pub timeout_seconds: Option<u64>,
    pub max_retries: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            spec_path: None,
            base_url: None,
            headers: HashMap::new(),
            timeout_seconds: Some(30),
            max_retries: Some(3),
        }
    }
}

impl Config {
    pub fn load(spec_path: Option<&str>) -> Result<Self> {
        let mut config: Config = Figment::new()
            .merge(Toml::file("anytype-mcp.toml"))
            .merge(Json::file("anytype-mcp.json"))
            .merge(Env::prefixed("ANYTYPE_MCP_"))
            .extract()
            .unwrap_or_default();

        // Handle legacy environment variable format
        if let Ok(headers_json) = std::env::var("OPENAPI_MCP_HEADERS") {
            if let Ok(headers) = serde_json::from_str::<HashMap<String, String>>(&headers_json) {
                config.headers.extend(headers);
            }
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
}
