use reqwest::{Client, Method, RequestBuilder, Response};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, warn, error};
use url::Url;
use base64::{Engine as _, engine::general_purpose};
use crate::config::Config;
use crate::openapi::McpTool;
use crate::utils::{AnytypeMcpError, Result as McpResult};

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    base_url: String,
    default_headers: HashMap<String, String>,
}

impl HttpClient {
    pub fn new(config: &Config, base_url: String) -> McpResult<Self> {
        let timeout = Duration::from_secs(config.timeout_seconds.unwrap_or(30));

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(AnytypeMcpError::HttpClient)?;

        Ok(Self {
            client,
            base_url,
            default_headers: config.headers.clone(),
        })
    }

    pub async fn execute_tool(
        &self,
        tool: &McpTool,
        params: Value,
    ) -> McpResult<Value> {
        debug!("Executing tool: {} with method: {} path: {}", tool.name, tool.method, tool.path);

        let url = self.build_url(&tool.path, &params)?;
        let method = self.parse_method(&tool.method)?;

        let mut request = self.client.request(method, url);

        // Add default headers
        for (key, value) in &self.default_headers {
            request = request.header(key, value);
        }

        // Handle request body and parameters
        request = match tool.method.to_uppercase().as_str() {
            "GET" | "DELETE" => {
                self.add_query_params(request, &params)?
            }
            "POST" | "PUT" | "PATCH" => {
                self.add_request_body(request, &params).await?
            }
            _ => request,
        };

        let response = request.send().await
            .map_err(AnytypeMcpError::HttpClient)?;

        self.handle_response(response).await
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

        Url::parse(&url_str)
            .map_err(|e| AnytypeMcpError::Config(format!("Invalid URL: {}", e)))
    }

    fn parse_method(&self, method: &str) -> McpResult<Method> {
        method.parse()
            .map_err(|e| AnytypeMcpError::Config(format!("Invalid HTTP method: {}", e)))
    }

    fn add_query_params(&self, mut request: RequestBuilder, params: &Value) -> McpResult<RequestBuilder> {
        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                if !key.starts_with('_') { // Skip internal parameters
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

    async fn add_request_body(&self, mut request: RequestBuilder, params: &Value) -> McpResult<RequestBuilder> {
        // Check if this is a file upload
        if let Some(obj) = params.as_object() {
            if obj.contains_key("_file_upload") {
                return self.add_multipart_body(request, params).await;
            }
        }

        // Regular JSON body
        request = request.header("Content-Type", "application/json");

        // Filter out path parameters and internal parameters
        let mut body_params = params.clone();
        if let Some(obj) = body_params.as_object_mut() {
            obj.retain(|key, _| !key.starts_with('_'));
        }

        Ok(request.json(&body_params))
    }

    async fn add_multipart_body(&self, request: RequestBuilder, params: &Value) -> McpResult<RequestBuilder> {
        let mut form = reqwest::multipart::Form::new();

        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                match key.as_str() {
                    "_file_upload" => {
                        // Handle file upload
                        if let Some(file_data) = value.as_str() {
                            // Decode base64 if needed, or handle as binary
                            let bytes = if file_data.starts_with("data:") {
                                // Handle data URLs
                                self.decode_data_url(file_data)?
                            } else {
                                // Assume it's a file path or base64
                                if std::path::Path::new(file_data).exists() {
                                    tokio::fs::read(file_data).await
                                        .map_err(AnytypeMcpError::Io)?
                                } else {
                                    // Try base64 decode
                                    general_purpose::STANDARD.decode(file_data)
                                        .map_err(|e| AnytypeMcpError::Validation(format!("Invalid file data: {}", e)))?
                                }
                            };

                            let part = reqwest::multipart::Part::bytes(bytes)
                                .file_name("upload")
                                .mime_str("application/octet-stream")
                                .map_err(|e| AnytypeMcpError::Config(format!("Invalid MIME type: {}", e)))?;

                            form = form.part("file", part);
                        }
                    }
                    key if !key.starts_with('_') => {
                        // Regular form field
                        let value_str = match value {
                            Value::String(s) => s.clone(),
                            _ => value.to_string().trim_matches('"').to_string(),
                        };
                        form = form.text(key.to_string(), value_str);
                    }
                    _ => {} // Skip internal parameters
                }
            }
        }

        Ok(request.multipart(form))
    }

    fn decode_data_url(&self, data_url: &str) -> McpResult<Vec<u8>> {
        // Parse data URL format: data:[<mediatype>][;base64],<data>
        if let Some(comma_pos) = data_url.find(',') {
            let data_part = &data_url[comma_pos + 1..];
            if data_url[..comma_pos].contains("base64") {
                general_purpose::STANDARD.decode(data_part)
                    .map_err(|e| AnytypeMcpError::Validation(format!("Invalid base64 data: {}", e)))
            } else {
                Ok(data_part.as_bytes().to_vec())
            }
        } else {
            Err(AnytypeMcpError::Validation("Invalid data URL format".to_string()))
        }
    }

    async fn handle_response(&self, response: Response) -> McpResult<Value> {
        let status = response.status();
        let headers = response.headers().clone();

        debug!("Response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!("HTTP error {}: {}", status, error_text);
            return Err(AnytypeMcpError::ToolExecution(
                format!("HTTP {} error: {}", status, error_text)
            ));
        }

        // Try to parse as JSON first
        let content_type = headers.get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if content_type.contains("application/json") {
            let text = response.text().await
                .map_err(AnytypeMcpError::HttpClient)?;

            if text.is_empty() {
                return Ok(Value::Null);
            }

            serde_json::from_str(&text)
                .map_err(|e| {
                    warn!("Failed to parse JSON response: {}", e);
                    AnytypeMcpError::Json(e)
                })
        } else {
            // For non-JSON responses, return as string
            let text = response.text().await
                .map_err(AnytypeMcpError::HttpClient)?;
            Ok(Value::String(text))
        }
    }
}
