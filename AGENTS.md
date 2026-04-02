# AGENTS.md

This file provides guidance to agents when working with code in this repository.

## Stack

- Language: Rust (Edition 2021)
- Package Manager: Cargo (workspace with 6 crates)
- Framework: Custom AI agent orchestration engine
- Key Dependencies: `tokio` (async runtime), `sqlx` (SQLite), `reqwest` (HTTP), `ratatui` (TUI), `petgraph` (DAG), `serde` (serialization)

## Workspace Structure

- `shared/` - Core data types, errors, config, brief contracts
- `plan/` - Plan parsing, validation, dependency graphs, marker tracking
- `index/` - Codebase indexing (Ollama embeddings + Qdrant vector store)
- `mcp/` - Multi-Context Protocol server implementation (JSON-RPC over stdio)
- `core/` - Agent orchestration, LLM client, patcher, session store
- `cli/` - CLI entry point + TUI (ratatui-based)

## Commands

- `cargo build` - Build all workspace crates
- `cargo test` - Run all tests (unit + integration)
- `cargo test --package telisq-core` - Run tests for specific crate
- `cargo test --test orchestrator_test` - Run single integration test
- `cargo run --bin telisq -- plan` - Run planning phase
- `cargo run --bin telisq -- run` - Run execution phase with TUI
- `cargo run --bin telisq -- index` - Index codebase artifacts
- `cargo run --bin telisq -- doctor` - Run diagnostics
- `cargo run --bin telisq -- bootstrap` - Create default config

## Code Style

- All files MUST have copyright header: `// Copyright 2026 Your Name.\n// SPDX-License-Identifier: MIT`
- `#![warn(missing_docs)]` and `#![forbid(unsafe_code)]` required for library crates
- Use `thiserror` for error types, `anyhow` for application errors
- Async code uses `tokio` with `#[tokio::main]` entry points
- Tracing (`tracing::*`) used for logging, not `log` or `println!`
- Serde derive macros for serialization; `#[serde(rename_all = "snake_case")]` for enums

## Non-Obvious Conventions

- Config loaded from `~/.telisq/config.yaml` (global) or `.telisq.toml` (project override)
- MCP servers communicate via JSON-RPC over stdio pipes (not HTTP)
- Index uses Ollama for embeddings (`http://localhost:11434`) and Qdrant for storage (`http://localhost:6334`)
- Session store uses SQLite via `sqlx` with schema versioning
- Plan files use markdown with special markers for task status tracking
- `AgentEvent::UserInputRequired` cannot be cloned due to `oneshot::Sender` (manual Clone impl converts it to Progress)
- TUI uses `ratatui` with event loop in `cli/src/tui/events.rs`
- Tests use `tempdir` crate (not `tempfile`) for integration test isolation
- `CARGO_MANIFEST_DIR` env var used in tests to locate fixture files

## Mode-Specific Rules

- [Ask Mode](.kilocode/rules-ask/AGENTS.md)
- [Code Mode](.kilocode/rules-code/AGENTS.md)
- [Architect Mode](.kilocode/rules-architect/AGENTS.md)
- [Debug Mode](.kilocode/rules-debug/AGENTS.md)
- [Review Mode](.kilocode/rules-review/AGENTS.md)
- [Orchestrator Mode](.kilocode/rules-orchestrator/AGENTS.md)

## Mode Capability Boundaries

- Ask: Read/analyze only. No file edits or destructive commands.
- Architect: Planning + documentation outputs. No source-code implementation.
- Code: Implementation/refactor with scoped file edits and verification commands.
- Debug: Reproduction + diagnosis + minimal fixes; avoid unrelated feature work.
- Review: Read-only audit and findings; no direct file edits.
- Orchestrator: Decomposition, sequencing, handoffs, and mode switching; no direct implementation unless explicitly delegated.

## Plan-Driven Execution Workflow

- Architect prepares design artifact and includes `Execution Inputs` for implementation handoff.
- Orchestrator creates detailed execution plan under `/plans/YYYY-MM-DD_<task-slug>.md`.
- Code mode reads the assigned plan before implementation and updates the same plan after each implementation batch.
- Required plan updates include checklist status, timestamped progress log, and changed-file summary.
- Task is not complete until plan status is `Done` with final summary.

## Serena Workflow

This project uses the Serena workflow for enhanced code intelligence:

### Available Workflows

- `/serena-setup.md` - Initial project setup and context loading
- `/serena-dev-workflow.md` - Complete development workflow
- `/serena-general.md` - General purpose workflow

### Core Workflow Steps

1. **LOAD CONTEXT** → `list_memories()` + `read_memory()` from Serena
2. **THINK** → Sequential Thinking for analysis
3. **LOOKUP** → Context7 if external libraries involved
4. **EXECUTE** → Serena tools for code operations
5. **VERIFY** → Check results
6. **SAVE** → `write_memory()` to Serena after completing work

### Memory Management

- ALWAYS load memories BEFORE starting any task
- ALWAYS save memories AFTER completing significant work
- Memory naming: `descriptive_name_YYYY_MM` (e.g., `auth_system_2025_01`)
