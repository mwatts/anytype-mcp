use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnytypeMcpError {
    #[error("OpenAPI specification error: {0}")]
    OpenApiSpec(String),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Tool execution failed: {0}")]
    ToolExecution(String),

    #[error("MCP protocol error: {0}")]
    McpProtocol(String),
}

pub type Result<T> = std::result::Result<T, AnytypeMcpError>;

impl From<AnytypeMcpError> for serde_json::Value {
    fn from(error: AnytypeMcpError) -> Self {
        serde_json::json!({
            "error": {
                "code": -32000,
                "message": error.to_string()
            }
        })
    }
}
