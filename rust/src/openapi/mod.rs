use crate::utils::{AnytypeMcpError, Result as McpResult};
use openapiv3::OpenAPI;
use std::path::Path;
use tracing::{debug, info};

pub mod parser;

pub use parser::{McpTool, OpenApiParser};

pub async fn load_openapi_spec<P: AsRef<Path>>(path: P) -> McpResult<OpenAPI> {
    let path = path.as_ref();
    info!("Loading OpenAPI specification from: {}", path.display());

    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(AnytypeMcpError::Io)?;

    let spec: OpenAPI = if path.extension().and_then(|s| s.to_str()) == Some("yaml")
        || path.extension().and_then(|s| s.to_str()) == Some("yml")
    {
        serde_yaml::from_str(&content)
            .map_err(|e| AnytypeMcpError::OpenApiSpec(format!("YAML parsing error: {}", e)))?
    } else {
        serde_json::from_str(&content).map_err(AnytypeMcpError::Json)?
    };

    validate_openapi_spec(&spec)?;
    debug!("Successfully loaded and validated OpenAPI specification");

    Ok(spec)
}

pub fn validate_openapi_spec(spec: &OpenAPI) -> McpResult<()> {
    // Basic validation
    if spec.openapi.is_empty() {
        return Err(AnytypeMcpError::Validation(
            "OpenAPI version is required".to_string(),
        ));
    }

    if !spec.openapi.starts_with("3.") {
        return Err(AnytypeMcpError::Validation(format!(
            "Unsupported OpenAPI version: {}. Only 3.x is supported",
            spec.openapi
        )));
    }

    if spec.info.title.is_empty() {
        return Err(AnytypeMcpError::Validation(
            "API title is required".to_string(),
        ));
    }

    if spec.paths.paths.is_empty() {
        return Err(AnytypeMcpError::Validation(
            "At least one path is required".to_string(),
        ));
    }

    debug!("OpenAPI specification validation passed");
    Ok(())
}

pub fn get_base_url(spec: &OpenAPI) -> Option<String> {
    spec.servers.first().map(|server| server.url.clone())
}
