use crate::config::Config;
use crate::openapi::McpTool;
use crate::utils::{AnytypeMcpError, Result as McpResult};
use base64::{Engine as _, engine::general_purpose};
use reqwest::{Client, Method, RequestBuilder, Response};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, warn};
use url::Url;

/// Anytype API version sent with every request; must match the bundled OpenAPI spec.
pub const ANYTYPE_API_VERSION: &str = "2025-11-08";

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    base_url: String,
    default_headers: HashMap<String, String>,
}

impl HttpClient {
    /// Create a new HTTP client with the given configuration and base URL.
    ///
    /// This client automatically includes the following headers in all requests:
    /// - `Anytype-Version` - Required Anytype API version header ([`ANYTYPE_API_VERSION`])
    /// - `Content-Type: application/json` - JSON content type for API requests
    /// - `Authorization: Bearer <api_key>` - Only if api_key is set in config
    ///
    /// The API key can be provided either in the config struct or via the `ANYTYPE_API_KEY` environment variable.
    pub fn new(config: &Config, base_url: String) -> McpResult<Self> {
        let timeout = Duration::from_secs(config.timeout_seconds.unwrap_or(30));

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(AnytypeMcpError::HttpClient)?;

        // Build default headers including required ones
        let mut default_headers = config.headers.clone();

        // Add required Anytype headers
        default_headers.insert(
            "Anytype-Version".to_string(),
            ANYTYPE_API_VERSION.to_string(),
        );
        default_headers.insert("Content-Type".to_string(), "application/json".to_string());

        // Add Authorization header if API key is provided
        if let Some(api_key) = &config.api_key {
            debug!("Adding Authorization header with Bearer token");
            default_headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
        } else {
            debug!("No API key provided, skipping Authorization header");
        }

        Ok(Self {
            client,
            base_url,
            default_headers,
        })
    }

    pub async fn execute_tool(&self, tool: &McpTool, params: Value) -> McpResult<Value> {
        debug!(
            "Executing tool: {} with method: {} path: {}",
            tool.name, tool.method, tool.path
        );

        let path_params = Self::path_param_names(&tool.path);
        let url = self.build_url(&tool.path, &params)?;
        let method = self.parse_method(&tool.method)?;

        let mut request = self.client.request(method, url);

        // Add default headers
        for (key, value) in &self.default_headers {
            request = request.header(key, value);
        }

        // Handle request body and parameters; path params are already
        // substituted into the URL and must not repeat in query or body.
        request = match tool.method.to_uppercase().as_str() {
            "GET" | "DELETE" => self.add_query_params(request, &params, &path_params)?,
            "POST" | "PUT" | "PATCH" => {
                self.add_request_body(request, tool, &params, &path_params)
                    .await?
            }
            _ => request,
        };

        let response = request.send().await.map_err(AnytypeMcpError::HttpClient)?;

        self.handle_response(response).await
    }

    /// Names of `{placeholder}` path parameters in an operation path.
    fn path_param_names(path: &str) -> Vec<String> {
        path.split('{')
            .skip(1)
            .filter_map(|part| part.split('}').next())
            .map(String::from)
            .collect()
    }

    fn build_url(&self, path: &str, params: &Value) -> McpResult<Url> {
        let mut url_str = format!("{}{}", self.base_url.trim_end_matches('/'), path);

        // Replace path parameters
        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                let placeholder = format!("{{{}}}", key);
                if url_str.contains(&placeholder) {
                    let value_str = match value {
                        Value::String(s) => s.clone(),
                        _ => value.to_string().trim_matches('"').to_string(),
                    };
                    url_str = url_str.replace(&placeholder, &value_str);
                }
            }
        }

        Url::parse(&url_str).map_err(|e| AnytypeMcpError::Config(format!("Invalid URL: {}", e)))
    }

    fn parse_method(&self, method: &str) -> McpResult<Method> {
        method
            .parse()
            .map_err(|e| AnytypeMcpError::Config(format!("Invalid HTTP method: {}", e)))
    }

    fn add_query_params(
        &self,
        mut request: RequestBuilder,
        params: &Value,
        path_params: &[String],
    ) -> McpResult<RequestBuilder> {
        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                if !key.starts_with('_') && !path_params.contains(key) {
                    let value_str = match value {
                        Value::String(s) => s.clone(),
                        Value::Null => continue,
                        _ => value.to_string().trim_matches('"').to_string(),
                    };
                    request = request.query(&[(key, value_str)]);
                }
            }
        }
        Ok(request)
    }

    async fn add_request_body(
        &self,
        mut request: RequestBuilder,
        tool: &McpTool,
        params: &Value,
        path_params: &[String],
    ) -> McpResult<RequestBuilder> {
        if !tool.file_upload_params.is_empty() {
            return self
                .add_multipart_body(request, tool, params, path_params)
                .await;
        }

        // Regular JSON body: everything except path and internal parameters
        request = request.header("Content-Type", "application/json");

        let mut body_params = params.clone();
        if let Some(obj) = body_params.as_object_mut() {
            obj.retain(|key, _| !key.starts_with('_') && !path_params.contains(key));
        }

        Ok(request.json(&body_params))
    }

    async fn add_multipart_body(
        &self,
        request: RequestBuilder,
        tool: &McpTool,
        params: &Value,
        path_params: &[String],
    ) -> McpResult<RequestBuilder> {
        let mut form = reqwest::multipart::Form::new();

        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                if key.starts_with('_') || path_params.contains(key) {
                    continue;
                }
                if tool.file_upload_params.contains(key) {
                    // File parameters accept a local file path or a list of paths.
                    let paths: Vec<&str> = match value {
                        Value::String(s) => vec![s.as_str()],
                        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
                        Value::Null => Vec::new(),
                        _ => {
                            return Err(AnytypeMcpError::Validation(format!(
                                "File path must be provided for parameter: {}",
                                key
                            )));
                        }
                    };
                    for path in paths {
                        form = form.part(key.clone(), self.file_part(path).await?);
                    }
                } else {
                    let value_str = match value {
                        Value::String(s) => s.clone(),
                        Value::Null => continue,
                        _ => value.to_string().trim_matches('"').to_string(),
                    };
                    form = form.text(key.clone(), value_str);
                }
            }
        }

        Ok(request.multipart(form))
    }

    /// Build a multipart part from a local file path (preferred), a data URL,
    /// or raw base64 content.
    async fn file_part(&self, file_data: &str) -> McpResult<reqwest::multipart::Part> {
        let (bytes, file_name) = if file_data.starts_with("data:") {
            (self.decode_data_url(file_data)?, "upload".to_string())
        } else if std::path::Path::new(file_data).exists() {
            let bytes = tokio::fs::read(file_data)
                .await
                .map_err(AnytypeMcpError::Io)?;
            let name = std::path::Path::new(file_data)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "upload".to_string());
            (bytes, name)
        } else {
            let bytes = general_purpose::STANDARD
                .decode(file_data)
                .map_err(|e| AnytypeMcpError::Validation(format!("Invalid file data: {}", e)))?;
            (bytes, "upload".to_string())
        };

        reqwest::multipart::Part::bytes(bytes)
            .file_name(file_name)
            .mime_str("application/octet-stream")
            .map_err(|e| AnytypeMcpError::Config(format!("Invalid MIME type: {}", e)))
    }

    fn decode_data_url(&self, data_url: &str) -> McpResult<Vec<u8>> {
        // Parse data URL format: data:[<mediatype>][;base64],<data>
        if let Some(comma_pos) = data_url.find(',') {
            let data_part = &data_url[comma_pos + 1..];
            if data_url[..comma_pos].contains("base64") {
                general_purpose::STANDARD
                    .decode(data_part)
                    .map_err(|e| AnytypeMcpError::Validation(format!("Invalid base64 data: {}", e)))
            } else {
                Ok(data_part.as_bytes().to_vec())
            }
        } else {
            Err(AnytypeMcpError::Validation(
                "Invalid data URL format".to_string(),
            ))
        }
    }

    async fn handle_response(&self, response: Response) -> McpResult<Value> {
        let status = response.status();
        let headers = response.headers().clone();

        debug!("Response status: {}", status);

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!("HTTP error {}: {}", status, error_text);
            return Err(AnytypeMcpError::ToolExecution(format!(
                "HTTP {} error: {}",
                status, error_text
            )));
        }

        // Try to parse as JSON first
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if content_type.contains("application/json") {
            let text = response.text().await.map_err(AnytypeMcpError::HttpClient)?;

            if text.is_empty() {
                return Ok(Value::Null);
            }

            serde_json::from_str(&text).map_err(|e| {
                warn!("Failed to parse JSON response: {}", e);
                AnytypeMcpError::Json(e)
            })
        } else {
            // For non-JSON responses, return as string
            let text = response.text().await.map_err(AnytypeMcpError::HttpClient)?;
            Ok(Value::String(text))
        }
    }

    /// Get the default headers for testing purposes
    #[cfg(test)]
    pub fn get_default_headers(&self) -> &HashMap<String, String> {
        &self.default_headers
    }
}
