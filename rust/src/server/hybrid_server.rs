use std::sync::Arc;

use anyhow::Result;
use tracing::{info, warn, error};

use crate::config::Config;
use crate::server::AnytypeJsonRpcServer;

/// Server mode enum to determine which transport to use
#[derive(Debug, Clone)]
pub enum ServerMode {
    /// Local JSON-RPC server using stdio transport
    JsonRpcStdio,
    /// Local JSON-RPC server using SSE transport
    JsonRpcSse { port: u16 },
    /// Local JSON-RPC server using Streamable HTTP transport
    JsonRpcStreamableHttp { port: u16 },
    /// Remote service calls via OpenAPI spec (legacy mode)
    #[allow(dead_code)]
    RemoteService,
}

/// Hybrid MCP server that supports both JSON-RPC protocol and remote service calls
pub struct HybridMcpServer {
    config: Arc<Config>,
    spec_path: Option<String>,
    mode: ServerMode,
}

impl HybridMcpServer {
    /// Create a new hybrid MCP server
    pub fn new(spec_path: Option<String>, config: Config, mode: ServerMode) -> Self {
        Self {
            config: Arc::new(config),
            spec_path,
            mode,
        }
    }

    /// Start the server in the configured mode
    pub async fn start(&self) -> Result<()> {
        info!("Starting Hybrid MCP Server in mode: {:?}", self.mode);

        match &self.mode {
            ServerMode::JsonRpcStdio => {
                info!("Starting JSON-RPC server with stdio transport");
                let server = AnytypeJsonRpcServer::new(
                    self.spec_path.clone(),
                    (*self.config).clone()
                ).await?;
                server.start_stdio().await
            },
            ServerMode::JsonRpcSse { port } => {
                info!("Starting JSON-RPC server with SSE transport on port {}", port);
                let server = AnytypeJsonRpcServer::new(
                    self.spec_path.clone(),
                    (*self.config).clone()
                ).await?;
                server.start_sse(*port).await
            },
            ServerMode::JsonRpcStreamableHttp { port } => {
                info!("Starting JSON-RPC server with Streamable HTTP transport on port {}", port);
                // For now, we'll use SSE as streamable HTTP requires additional implementation
                warn!("Streamable HTTP transport not yet implemented, falling back to SSE");
                let server = AnytypeJsonRpcServer::new(
                    self.spec_path.clone(),
                    (*self.config).clone()
                ).await?;
                server.start_sse(*port).await
            },
            ServerMode::RemoteService => {
                warn!("Remote service mode is deprecated and will be removed in future versions");
                error!("Legacy remote service mode is no longer supported");
                Err(anyhow::anyhow!("Remote service mode is deprecated. Please use JSON-RPC mode instead."))
            }
        }
    }

    /// Get server information and capabilities
    pub async fn get_server_info(&self) -> Result<String> {
        match &self.mode {
            ServerMode::JsonRpcStdio | ServerMode::JsonRpcSse { .. } | ServerMode::JsonRpcStreamableHttp { .. } => {
                let server = AnytypeJsonRpcServer::new(
                    self.spec_path.clone(),
                    (*self.config).clone()
                ).await?;

                let info = server.get_info();
                Ok(format!(
                    "Server: {} v{}\nMode: {:?}\nCapabilities: {:?}\nTools: {}\n",
                    info.server_info.name,
                    info.server_info.version,
                    self.mode,
                    info.capabilities,
                    server.get_tools().len()
                ))
            },
            ServerMode::RemoteService => {
                Ok("Remote service mode is deprecated".to_string())
            }
        }
    }

    /// List available tools
    pub async fn list_tools(&self) -> Result<Vec<String>> {
        match &self.mode {
            ServerMode::JsonRpcStdio | ServerMode::JsonRpcSse { .. } | ServerMode::JsonRpcStreamableHttp { .. } => {
                let server = AnytypeJsonRpcServer::new(
                    self.spec_path.clone(),
                    (*self.config).clone()
                ).await?;

                Ok(server.get_tools().iter().map(|tool| tool.name.clone()).collect())
            },
            ServerMode::RemoteService => {
                Ok(vec!["Remote service mode is deprecated".to_string()])
            }
        }
    }

    /// Validate configuration and server setup
    pub async fn validate(&self) -> Result<()> {
        info!("Validating hybrid MCP server configuration");

        // Validate spec path if provided
        if let Some(path) = &self.spec_path {
            if path.starts_with("http") {
                info!("OpenAPI spec will be loaded from URL: {}", path);
            } else if !std::path::Path::new(path).exists() {
                return Err(anyhow::anyhow!("OpenAPI spec file not found: {}", path));
            }
        }

        // Validate mode-specific configuration
        match &self.mode {
            ServerMode::JsonRpcStdio => {
                info!("Stdio transport validated");
            },
            ServerMode::JsonRpcSse { port } | ServerMode::JsonRpcStreamableHttp { port } => {
                if *port == 0 {
                    return Err(anyhow::anyhow!("Invalid port number: {}", port));
                }
                info!("HTTP transport on port {} validated", port);
            },
            ServerMode::RemoteService => {
                return Err(anyhow::anyhow!("Remote service mode is deprecated"));
            }
        }

        // Create server to validate configuration
        let server = AnytypeJsonRpcServer::new(
            self.spec_path.clone(),
            (*self.config).clone()
        ).await?;

        info!("Server configuration valid with {} tools", server.get_tools().len());
        Ok(())
    }
}

impl Default for ServerMode {
    fn default() -> Self {
        ServerMode::JsonRpcStdio
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_hybrid_server_creation() {
        let config = Config::default();
        let server = HybridMcpServer::new(None, config, ServerMode::JsonRpcStdio);

        let validation_result = server.validate().await;
        assert!(validation_result.is_ok());
    }

    #[tokio::test]
    async fn test_server_info() {
        let config = Config::default();
        let server = HybridMcpServer::new(None, config, ServerMode::JsonRpcStdio);

        let info = server.get_server_info().await;
        assert!(info.is_ok());
        assert!(info.unwrap().contains("anytype-mcp-server"));
    }

    #[tokio::test]
    async fn test_list_tools() {
        let config = Config::default();
        let server = HybridMcpServer::new(None, config, ServerMode::JsonRpcStdio);

        let tools = server.list_tools().await;
        assert!(tools.is_ok());
        // Should load tools from embedded OpenAPI spec by default
        let tool_count = tools.unwrap().len();
        assert!(tool_count > 0, "Expected tools to be loaded from embedded spec, got: {}", tool_count);
    }

    #[test]
    fn test_invalid_port() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let config = Config::default();
            let server = HybridMcpServer::new(None, config, ServerMode::JsonRpcSse { port: 0 });

            let validation_result = server.validate().await;
            assert!(validation_result.is_err());
        });
    }

    #[test]
    fn test_remote_service_deprecated() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let config = Config::default();
            let server = HybridMcpServer::new(None, config, ServerMode::RemoteService);

            let validation_result = server.validate().await;
            assert!(validation_result.is_err());
        });
    }
}
