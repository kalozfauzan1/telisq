# Project Coding Rules (Non-Obvious Only)

## Capability Boundary (Enforced)

- Scope: Implement features, refactor, and maintain code quality for requested scope.
- Allowed operations: create/edit/delete source and test files, run verification commands.
- Required: keep changes scoped, preserve backward compatibility unless user approved breaking changes.
- Forbidden operations: broad unrelated refactors, architecture-only planning without implementation intent.

## Mandatory Tool Usage

- **Sequential Thinking**: Always use before implementing complex features or refactoring
- **Context7**: Always query for library documentation when using:
  - Framework-specific patterns and features
  - Language-specific type system features
  - Framework-specific styling integration

## Requirement Clarification Policy

- **ALWAYS ask before implementing** when:

  - File paths or targets are unclear
  - Expected behavior has multiple interpretations
  - Edge cases are not specified
  - Changes might affect existing functionality

- **Use `ask_followup_question` tool** with 2-4 suggested implementation approaches
- **Never assume** - confirm understanding of requirements before writing code

## Mandatory Plan Progress Updates (/plans)

- Code mode MUST read the assigned plan file under `/plans/` before implementing.
- After each meaningful implementation batch (one or more file edits), code mode MUST update the same plan file with:
  - checklist item state change
  - timestamped progress log entry
  - short note of changed files and what was implemented
- When implementation is complete, code mode MUST:
  - mark remaining completed checklist items
  - set final status to `Done`
  - add a concise final summary
- If code changes are made but `/plans` progress is not updated, the task is considered incomplete.

## Import Order

Standard Rust import order: std → external crates → workspace crates (shared, plan, index, mcp, core) → local modules.

## Path Aliases

No path aliases - Rust uses relative imports. Workspace crates reference each other via `Cargo.toml` dependencies.

## Code Style

- All files MUST have copyright header: `// Copyright 2026 Your Name.\n// SPDX-License-Identifier: MIT`
- Library crates require `#![warn(missing_docs)]` and `#![forbid(unsafe_code)]`
- Use `thiserror` for error types, `anyhow` for application errors
- Async code uses `tokio` with `#[tokio::main]` entry points
- Tracing (`tracing::*`) used for logging, not `log` or `println!`
- Serde derive macros for serialization; `#[serde(rename_all = "snake_case")]` for enums
- Error types use `#[derive(Error, Debug)]` with `#[error("...")]` annotations

## Project-Specific Patterns

- **Agent Briefs**: Agents receive typed briefs from `shared::brief` (PlanBrief, CodeBrief, ReviewBrief, AskBrief)
- **Agent Runner Trait**: All agents implement `AgentRunner` trait with `async fn run()` returning `AgentResult`
- **AgentEvent Clone Limitation**: `AgentEvent::UserInputRequired` cannot be cloned due to `oneshot::Sender` - manual Clone impl converts it to Progress
- **Patcher**: Uses simple string replacement via `Patcher::apply_patch()` - original content must match exactly
- **Task Graph**: Uses `petgraph` for DAG-based task dependency resolution with topological sorting
- **Session Store**: SQLite via `sqlx` with schema versioning (`SCHEMA_VERSION` constant)
- **MCP Protocol**: JSON-RPC over stdio pipes (not HTTP) - servers spawned as child processes
- **Index System**: Ollama for embeddings (`http://localhost:11434`), Qdrant for vector storage (`http://localhost:6334`)
- **Config Loading**: Global config at `~/.telisq/config.yaml`, project override at `.telisq.toml`
- **Plan Files**: Markdown format with special markers for task status tracking (`- [ ]`, `- [x]`, `- [-]`, etc.)

## Testing Conventions

- Tests use `tempdir` crate (not `tempfile`) for integration test isolation
- `CARGO_MANIFEST_DIR` env var used in tests to locate fixture files
- Test fixtures in `tests/fixtures/` directory
- Unit tests in `tests/unit/`, integration tests in `tests/integration/`
- Mock LLM/MCP tests use custom mock implementations (not mockito)

## Serena Workflow (Mandatory)

**ALWAYS follow this workflow when working with code:**

1. **LOAD CONTEXT** → `list_memories()` + `read_memory()` from Serena
2. **THINK** → Sequential Thinking for analysis
3. **LOOKUP** → Context7 if external libraries involved
4. **EXECUTE** → Serena tools for code operations
5. **VERIFY** → Check results
6. **SAVE** → `write_memory()` to Serena after completing work

**Memory Rules:**

- ALWAYS load memories BEFORE starting any task
- ALWAYS save memories AFTER completing significant work
- Keep memories CONCISE — focus on key points
- Memory naming: `descriptive_name_YYYY_MM` (e.g., `auth_system_2025_01`)
