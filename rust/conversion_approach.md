# Anytype MCP Server: TypeScript to Rust Conversion Approach

## Overview

This document outlines the approach for converting the TypeScript-based Anytype MCP Server to Rust using the [MCPR library](https://github.com/conikeec/mcpr) (Model Context Protocol for Rust).

## Current TypeScript Architecture Analysis

### Core Components

1. **OpenAPI to MCP Converter** (`src/openapi/parser.ts`)
   - Converts OpenAPI 3.x specifications to MCP tools
   - Handles schema resolution and JSON Schema conversion
   - Manages complex type mappings and references

2. **HTTP Client** (`src/client/http-client.ts`)
   - Handles HTTP requests with authentication
   - Supports file uploads and multipart forms
   - Implements retry logic and error handling

3. **MCP Proxy** (`src/mcp/proxy.ts`)
   - Acts as the main MCP server implementation
   - Bridges OpenAPI operations to MCP tool calls
   - Handles dynamic tool registration

4. **Authentication System** (`src/auth/`)
   - Manages API key generation and templates
   - Provides interactive key setup functionality

5. **Initialization and Server Setup** (`src/init-server.ts`)
   - Loads and validates OpenAPI specifications
   - Initializes the MCP proxy server
   - Handles configuration and startup logic

### Key Features
- Dynamic tool generation from OpenAPI specs
- Support for complex parameter validation
- File upload capabilities
- Interactive API key generation
- CLI interface with multiple commands
- Comprehensive error handling

## Target Rust Architecture

### Dependencies and Crates

Based on the MCPR library analysis, we'll use:

- **mcpr** (v0.2.3+): Core MCP protocol implementation
- **tokio**: Async runtime for handling concurrent operations
- **serde**: JSON serialization/deserialization
- **reqwest**: HTTP client for API requests
- **openapi**: OpenAPI specification parsing
- **clap**: Command-line argument parsing
- **anyhow**: Error handling
- **tracing**: Structured logging

### Proposed Module Structure

```
rust/
├── Cargo.toml
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── lib.rs                  # Library exports
│   ├── server/
│   │   ├── mod.rs              # Server module
│   │   ├── mcp_server.rs       # Main MCP server implementation
│   │   └── tool_registry.rs    # Dynamic tool registration
│   ├── openapi/
│   │   ├── mod.rs              # OpenAPI module
│   │   ├── parser.rs           # OpenAPI to MCP converter
│   │   ├── schema_converter.rs # JSON Schema conversion
│   │   └── validator.rs        # OpenAPI spec validation
│   ├── client/
│   │   ├── mod.rs              # HTTP client module
│   │   ├── http_client.rs      # HTTP request handling
│   │   └── multipart.rs        # File upload support
│   ├── auth/
│   │   ├── mod.rs              # Authentication module
│   │   ├── key_generator.rs    # API key generation
│   │   └── interactive.rs      # Interactive setup
│   ├── config/
│   │   ├── mod.rs              # Configuration module
│   │   └── settings.rs         # Application settings
│   └── utils/
│       ├── mod.rs              # Utilities module
│       └── error.rs            # Error types and handling
├── examples/
│   └── basic_usage.rs          # Usage examples
└── tests/
    ├── integration/
    └── unit/
```

## Conversion Strategy

### Phase 1: Foundation Setup
1. **Project Structure**: Set up Cargo project with MCPR dependency
2. **Error Handling**: Define comprehensive error types using `anyhow` and `thiserror`
3. **Logging**: Implement structured logging with `tracing`
4. **Configuration**: Create configuration management system

### Phase 2: Core Components

#### OpenAPI Parser (`openapi/parser.rs`)
```rust
pub struct OpenApiToMcpConverter {
    spec: OpenApi,
    schema_cache: HashMap<String, serde_json::Value>,
}

impl OpenApiToMcpConverter {
    pub fn new(spec: OpenApi) -> Self { /* ... */ }

    pub fn convert_to_tools(&self) -> Result<Vec<Tool>, ConversionError> {
        // Convert OpenAPI operations to MCP tools
    }

    fn convert_schema(&self, schema: &Schema) -> Result<serde_json::Value, ConversionError> {
        // Convert OpenAPI schemas to JSON Schema
    }
}
```

#### HTTP Client (`client/http_client.rs`)
```rust
pub struct HttpClient {
    client: reqwest::Client,
    base_url: String,
    default_headers: HeaderMap,
}

impl HttpClient {
    pub async fn execute_request(
        &self,
        operation: &OpenApiOperation,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, HttpError> {
        // Execute HTTP requests based on OpenAPI operations
    }

    pub async fn upload_file(
        &self,
        operation: &OpenApiOperation,
        file_data: Vec<u8>,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, HttpError> {
        // Handle multipart file uploads
    }
}
```

#### MCP Server (`server/mcp_server.rs`)
```rust
pub struct AnytypeMcpServer {
    converter: OpenApiToMcpConverter,
    http_client: HttpClient,
    tools: Vec<Tool>,
}

impl AnytypeMcpServer {
    pub async fn new(openapi_spec_path: &str) -> Result<Self, ServerError> {
        // Initialize server with OpenAPI spec
    }

    pub async fn start(&mut self) -> Result<(), ServerError> {
        // Start MCP server using MCPR
        let config = ServerConfig::new()
            .with_name("Anytype MCP Server")
            .with_version("1.0.0");

        let mut server = Server::new(config);

        // Register dynamic tools
        for tool in &self.tools {
            server.register_tool_handler(&tool.name, |params| {
                // Handle tool execution
            })?;
        }

        let transport = StdioTransport::new();
        server.start(transport).await
    }
}
```

### Phase 3: Advanced Features

#### Authentication System
- Port the interactive API key generation functionality
- Implement secure key storage and retrieval
- Create user-friendly setup wizard

#### File Upload Support
- Implement multipart form data handling
- Support various content types
- Handle large file uploads efficiently

#### CLI Interface
```rust
#[derive(Parser)]
#[command(name = "anytype-mcp")]
#[command(about = "Anytype MCP Server")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(long)]
        spec_path: Option<String>,
    },
    GetKey {
        #[arg(long)]
        spec_path: Option<String>,
    },
}
```

### Phase 4: Testing and Validation

#### Unit Tests
- Test OpenAPI schema conversion
- Validate HTTP client functionality
- Test tool registration and execution

#### Integration Tests
- End-to-end MCP communication
- Real API interaction tests
- File upload scenarios

#### Performance Testing
- Benchmark against TypeScript version
- Memory usage optimization
- Concurrent request handling

## Key Challenges and Solutions

### 1. OpenAPI Schema Complexity
**Challenge**: Complex nested schemas and references in OpenAPI specs
**Solution**:
- Implement robust reference resolution with cycle detection
- Use recursive schema parsing with memoization
- Provide detailed error messages for invalid schemas

### 2. Dynamic Tool Registration
**Challenge**: MCPR requires compile-time tool registration
**Solution**:
- Use dynamic dispatch with trait objects
- Implement tool factory pattern
- Create macro system for tool generation

### 3. Async HTTP Operations
**Challenge**: Balancing async performance with MCP protocol requirements
**Solution**:
- Use Tokio for async runtime
- Implement connection pooling
- Handle timeouts and retries gracefully

### 4. Error Handling Consistency
**Challenge**: Maintaining consistent error handling across components
**Solution**:
- Define custom error types with `thiserror`
- Implement error conversion between layers
- Provide meaningful error messages to users

## Migration Benefits

### Performance Improvements
- **Memory Efficiency**: Rust's zero-cost abstractions and ownership model
- **Concurrency**: Native async/await with Tokio runtime
- **Startup Time**: Compiled binary with faster cold starts

### Reliability Enhancements
- **Type Safety**: Compile-time guarantees prevent runtime errors
- **Memory Safety**: No null pointer dereferences or buffer overflows
- **Error Handling**: Explicit error handling with Result types

### Operational Benefits
- **Single Binary**: No runtime dependencies or package installations
- **Cross-Platform**: Easy deployment across different architectures
- **Resource Usage**: Lower memory footprint and CPU usage

## Implementation Timeline

### Week 1-2: Foundation
- Set up Rust project structure
- Implement basic error handling and logging
- Create configuration system

### Week 3-4: Core Components
- Implement OpenAPI parser
- Create HTTP client with authentication
- Basic MCP server setup

### Week 5-6: Advanced Features
- Add file upload support
- Implement CLI interface
- Create authentication system

### Week 7-8: Testing and Polish
- Comprehensive test coverage
- Performance optimization
- Documentation and examples

## Compatibility Considerations

### API Compatibility
- Maintain identical MCP tool interfaces
- Preserve all existing functionality
- Ensure seamless client migration

### Configuration Compatibility
- Support existing environment variable format
- Maintain CLI command compatibility
- Preserve configuration file structure

### Deployment Compatibility
- Provide npm wrapper for easy migration
- Support existing CI/CD pipelines
- Maintain Docker deployment options

## Success Criteria

1. **Functional Parity**: All TypeScript features replicated in Rust
2. **Performance Improvement**: Measurable gains in memory usage and response times
3. **Reliability**: Reduced error rates and improved stability
4. **Maintainability**: Clear, well-documented code structure
5. **User Experience**: Seamless migration path for existing users

## Next Steps

1. Review and approve this conversion approach
2. Set up the initial Rust project structure
3. Begin implementing the OpenAPI parser component
4. Create initial test suite for validation
5. Establish CI/CD pipeline for Rust development

This approach ensures a systematic, well-planned conversion that maintains compatibility while leveraging Rust's performance and safety benefits.
