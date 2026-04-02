# Telisq Project Overview

## Purpose
Telisq is a structured planning and execution engine for software projects, designed to provide a reliable and efficient way to manage complex development workflows.

## Key Features
- Structured Planning: Create and manage detailed plans with dependencies and phases
- Intelligent Execution: Automate task execution with built-in agents
- Real-time Feedback: Visualize progress and get instant feedback through the TUI
- Session Management: Save and resume work from any point
- Codebase Indexing: Semantic search and knowledge base for your project
- LLM Integration: Leverage AI models for planning and problem-solving
- MCP Support: Extend functionality with Multi-Context Protocol tools

## Tech Stack
- Language: Rust (Edition 2021)
- Package Manager: Cargo (workspace with 6 crates)
- Framework: Custom AI agent orchestration engine
- Key Dependencies:
  - `tokio` (async runtime)
  - `sqlx` (SQLite)
  - `reqwest` (HTTP)
  - `ratatui` (TUI)
  - `petgraph` (DAG)
  - `serde` (serialization)
  - `thiserror` (error types)
  - `anyhow` (application errors)
  - `tracing` (logging)

## Workspace Structure
- `shared/` - Core data types, errors, config, brief contracts
- `plan/` - Plan parsing, validation, dependency graphs, marker tracking
- `index/` - Codebase indexing (Ollama embeddings + Qdrant vector store)
- `mcp/` - Multi-Context Protocol server implementation (JSON-RPC over stdio)
- `core/` - Agent orchestration, LLM client, patcher, session store
- `cli/` - CLI entry point + TUI (ratatui-based)

## External Services
- Ollama for embeddings at `http://localhost:11434`
- Qdrant for vector storage at `http://localhost:6334`
- Config at `~/.telisq/config.yaml` (global) or `.telisq.toml` (project override)