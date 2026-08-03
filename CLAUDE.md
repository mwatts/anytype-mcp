# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Always load the `rust` skill at the start of each session when working in this project.

@AGENTS.md

## Branch Context

This branch (`conversion/rust`) is a Rust port of the TypeScript MCP server. The active implementation lives in `rust/`; the original TypeScript in `src/` is frozen at the branch point and serves as the reference implementation. When porting behavior, compare against `main:src/` (which is newer), not the local `src/`. Feature-parity gaps are tracked as beads issues (labels `rust`, `parity`).

## Common Commands

All Rust work happens in `rust/`:

- `cargo build --all-targets` - Build library, binary, and examples
- `cargo test` - Run all tests
- `cargo clippy --all-targets` - Lint (keep warning-free)
- `cargo fmt` - Format code
- `cargo run -- list-tools` - Print the MCP tools derived from the OpenAPI spec
- `cargo run -- get-key` - API key acquisition helper
- `cargo run` - Start the MCP server on stdio

Toolchain: Rust edition 2024, MSRV 1.85. MCP SDK: `rmcp` 3.x.

## Architecture Overview

An MCP (Model Context Protocol) server that bridges Anytype's local API with AI assistants. OpenAPI specifications are dynamically converted to MCP tools at runtime — no tools are hardcoded.

### Core Components (rust/src/)

**Server (`server/json_rpc_server.rs`)**
- `AnytypeJsonRpcServer` implements `rmcp::ServerHandler` (`get_info`, `list_tools`, `call_tool`)
- Loads the OpenAPI spec with priority: `--spec-path` (file or URL) → `scripts/openapi.json` → `openapi.json` → embedded spec (`include_str!` of `scripts/openapi.json`) → remote fetch
- `server/hybrid_server.rs` is a thin facade selecting the transport mode; only stdio is functional

**OpenAPI Parser (`openapi/parser.rs`)**
- Converts OpenAPI paths/operations to `McpTool` values (name, description, input schema, method, path)
- One tool per operation across GET/POST/PUT/DELETE/PATCH

**HTTP Client (`client/http_client.rs`)**
- reqwest-based; substitutes `{param}` path placeholders, maps params to query string (GET/DELETE) or JSON body (POST/PUT/PATCH)
- Sends `Anytype-Version` and `Authorization: Bearer <key>` headers on every request

**Config (`config/mod.rs`)**
- Figment-layered: `anytype-mcp.toml` → `anytype-mcp.json` → `ANYTYPE_MCP_*` env vars
- Also honors `ANYTYPE_API_KEY` and legacy `OPENAPI_MCP_HEADERS` (JSON blob of headers)

### Key Design Patterns

1. **Dynamic tool generation**: tools are derived from the OpenAPI spec at runtime, so the server adapts to API changes without code edits.
2. **Header injection**: auth and version headers come from config/env, never from tool arguments.
3. **stdout is the protocol**: the server speaks MCP over stdio; all logging goes to stderr via `tracing`.

## Testing Strategy

Unit tests are colocated in `#[cfg(test)]` modules (`client/mod.rs`, `server/mod.rs`, `server/hybrid_server.rs`). Note `test_http_client_simple` performs a live request to httpbin.org. Verify changes with the stdio smoke test: pipe `initialize` / `tools/list` JSON-RPC lines into the binary and check the tool count.

## Issue Tracking

Use `br` (beads) for issue tracking — see the Beads Workflow Integration section imported from AGENTS.md above. Check `br ready --json` before starting work.
