# Code Style and Conventions

## General Rules
- All files MUST have copyright header: `// Copyright 2026 Your Name.\n// SPDX-License-Identifier: MIT`
- Library crates require `#![warn(missing_docs)]` and `#![forbid(unsafe_code)]`
- Edition 2021

## Error Handling
- Use `thiserror` for error types with `#[derive(Error, Debug)]` and `#[error("...")]` annotations
- Use `anyhow` for application errors

## Async Code
- Use `tokio` with `#[tokio::main]` entry points
- All async code uses tokio runtime

## Logging
- Use `tracing` crate (not `log` or `println!`)
- Set `RUST_LOG` env var to control log level

## Serialization
- Use serde derive macros
- `#[serde(rename_all = "snake_case")]` for enums

## Import Order
Standard Rust import order: std → external crates → workspace crates (shared, plan, index, mcp, core) → local modules

## Testing
- Tests use `tempdir` crate (not `tempfile`) for integration test isolation
- `CARGO_MANIFEST_DIR` env var used in tests to locate fixture files
- Test fixtures in `tests/fixtures/` directory
- Unit tests in `tests/unit/`, integration tests in `tests/integration/`
- Mock LLM/MCP tests use custom mock implementations (not mockito)

## Key Patterns
- **Agent Briefs**: Agents receive typed briefs from `shared::brief` (PlanBrief, CodeBrief, ReviewBrief, AskBrief)
- **Agent Runner Trait**: All agents implement `AgentRunner` trait with `async fn run()` returning `AgentResult`
- **AgentEvent Clone Limitation**: `AgentEvent::UserInputRequired` cannot be cloned due to `oneshot::Sender`
- **Patcher**: Uses simple string replacement via `Patcher::apply_patch()` - original content must match exactly
- **Task Graph**: Uses `petgraph` for DAG-based task dependency resolution
- **Session Store**: SQLite via `sqlx` with schema versioning
- **MCP Protocol**: JSON-RPC over stdio pipes (not HTTP)
- **Plan Files**: Markdown format with special markers (`- [ ]`, `- [x]`, `- [-]`)