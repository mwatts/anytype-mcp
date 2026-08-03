# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Always load the `rust` skill at the start of each session when working in this project.

@AGENTS.md

## Branch Context

This branch (`conversion/rust`) is a Rust port of the TypeScript MCP server. **All active code changes go to `rust/`**; the TypeScript in `src/` is the reference implementation (kept in sync via merges from `main`) and should not be modified here. Feature-parity gaps are tracked as beads issues (labels `rust`, `parity`).

## Common Commands

Tasks are managed with [mise](https://mise.jdx.dev) (`mise.toml` at the repo root pins the Rust toolchain and defines tasks; run from anywhere in the repo):

- `mise run ci` - Full quality gate: fmt-check, lint (clippy -D warnings), build, test
- `mise run build` / `mise run test` / `mise run fmt` / `mise run lint`
- `mise run list-tools` - Print the MCP tools derived from the OpenAPI spec (34)
- `mise run smoke` - MCP stdio smoke test (initialize + tools/list; prints 2 on success)
- `mise run get-key` - Interactive API key acquisition
- `mise run run` - Start the MCP server on stdio

Plain `cargo` from `rust/` works too. Toolchain: Rust edition 2024, MSRV 1.85. MCP SDK: `rmcp` 3.x.

For the TypeScript reference implementation (Bun): `bun install`, `bun run test`, `bun run build`. See `src/` for behavior comparisons — notably `src/openapi/parser.ts`, `src/mcp/proxy.ts`, `src/client/http-client.ts`, and `src/auth/get-key.ts`.

## Architecture Overview

An MCP (Model Context Protocol) server that bridges Anytype's local API with AI assistants. OpenAPI specifications are dynamically converted to MCP tools at runtime — no tools are hardcoded.

### Core Components (rust/src/)

**Server (`server/json_rpc_server.rs`)**
- `AnytypeJsonRpcServer` implements `rmcp::ServerHandler` (`get_info`, `list_tools`, `call_tool`)
- Loads the OpenAPI spec with priority: `--spec-path` (file or URL) → `scripts/openapi.json` → `openapi.json` → embedded spec (`include_str!` of `scripts/openapi.json`) → remote fetch
- `server/hybrid_server.rs` is a thin facade selecting the transport mode

**OpenAPI Parser (`openapi/parser.rs`)**
- Converts OpenAPI paths/operations to `McpTool` values; resolves `$ref`s with cycle detection
- Tool names follow the TS convention: `API-<kebab-case-operation-id>`, 64-char limit
- Excludes Auth-tagged operations and unsupported `filters` params; special-cases icon and property-value unions

**HTTP Client (`client/http_client.rs`)**
- reqwest-based; substitutes `{param}` path placeholders, maps params to query string (GET/DELETE) or JSON body (POST/PUT/PATCH)
- Sends `Anytype-Version` (see `ANYTYPE_API_VERSION`) and `Authorization: Bearer <key>` headers on every request

**Config (`config/mod.rs`)**
- Figment-layered: `anytype-mcp.toml` → `anytype-mcp.json` → `ANYTYPE_MCP_*` env vars
- Also honors `ANYTYPE_API_KEY` and legacy `OPENAPI_MCP_HEADERS` (JSON blob of headers)

### Key Design Patterns

1. **Dynamic tool generation**: tools are derived from the OpenAPI spec at runtime, so the server adapts to API changes without code edits.
2. **Header injection**: auth and version headers come from config/env, never from tool arguments.
3. **stdout is the protocol**: the server speaks MCP over stdio; all logging goes to stderr via `tracing`.

## Testing Strategy

Unit tests are colocated in `#[cfg(test)]` modules (`client/mod.rs`, `server/mod.rs`, `server/hybrid_server.rs`). Note `test_http_client_simple` performs a live request to httpbin.org. Verify changes with the stdio smoke test: pipe `initialize` / `tools/list` JSON-RPC lines into the binary and check the tool count (34).

## Issue Tracking

Use `br` (beads) for issue tracking — see the Beads Workflow Integration section imported from AGENTS.md above. Check `br ready --json` before starting work.
