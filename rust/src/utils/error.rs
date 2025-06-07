use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnytypeMcpError {
    #[error("OpenAPI specification error: {0}")]
    OpenApiSpec(String),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("MCP protocol error: {0}")]
    McpProtocol(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    #[error("MCPR protocol error: {0}")]
    McprProtocol(String),
}

pub type Result<T> = std::result::Result<T, AnytypeMcpError>;

impl From<mcpr::error::MCPError> for AnytypeMcpError {
    fn from(error: mcpr::error::MCPError) -> Self {
        AnytypeMcpError::McprProtocol(error.to_string())
    }
}

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
