# Project Architecture Rules (Non-Obvious Only)

## Capability Boundary (Enforced)

- Scope: System design, planning, trade-off analysis, and architecture documentation.
- Allowed operations: read/search/inspect and markdown/doc outputs.
- Forbidden operations: source-code implementation, runtime behavior changes, dependency installs.
- Handoff trigger: If user asks to build/modify code, switch to Code mode first.

## Mandatory Tool Usage

- **Sequential Thinking**: Always use for breaking down complex architecture problems
- **Context7**: Always query for:
  - Latest best practices for your framework/language
  - Framework-specific patterns and optimizations
  - State management best practices for your tech stack

## Requirement Clarification Policy

- **ALWAYS ask before planning** when:

  - Business requirements are unclear
  - Trade-off preferences are unknown (cost vs. performance vs. speed)
  - Scale requirements are missing
  - Integration points are not specified

- **Use `ask_followup_question` tool** with:
  - 2-4 architectural approaches with trade-offs
  - Mode switch suggestion: "switch to code mode to implement?"

## Mandatory Deliverable: Design Artifact + Execution Inputs

- For non-trivial tasks, produce a design artifact before orchestration begins.
- The design artifact MUST include:
  - Problem statement and scope boundaries
  - Options considered + selected approach with trade-offs
  - Interface/contract expectations and affected areas
  - Risks, assumptions, and acceptance criteria
- Add a dedicated section titled `Execution Inputs` that is optimized for orchestrator handoff.
- `Execution Inputs` MUST contain:
  - Milestones that can become checklist items in `/plans/<task>.md`
  - Expected file impact areas (high-level only)
  - Validation requirements (tests/checks)
  - Rollback/mitigation notes
- Architect MUST NOT edit implementation files; architect outputs design and handoff-ready documentation only.

## Architecture Constraints

- **Workspace Crates**: 6 crates with clear boundaries - shared (types), plan (parsing), index (embeddings), mcp (protocol), core (orchestration), cli (interface)
- **Agent Orchestration**: Orchestrator in `core/src/orchestrator.rs` manages agent lifecycle and task dispatch
- **Agent Communication**: Agents communicate via typed briefs (`shared::brief`) - not direct function calls
- **Task Execution**: DAG-based task execution using `petgraph` with topological sort for dependency ordering
- **LLM Integration**: OpenAI-compatible API via `reqwest` with retry logic and SSE streaming support
- **MCP Protocol**: JSON-RPC over stdio pipes - servers spawned as child processes, not HTTP endpoints
- **Index System**: Ollama (embeddings at `localhost:11434`) + Qdrant (vector store at `localhost:6334`) - external services
- **Session Persistence**: SQLite via `sqlx` with schema versioning - not in-memory
- **TUI Framework**: `ratatui` for terminal UI with event loop in `cli/src/tui/events.rs`
- **Error Handling**: `thiserror` for library errors, `anyhow` for application errors - consistent across crates
- **Async Runtime**: `tokio` with `#[tokio::main]` entry points - all async code uses this runtime
- **Config Loading**: Global config (`~/.telisq/config.yaml`) with project override (`.telisq.toml`)
- **Plan Format**: Markdown files with special markers (`- [ ]`, `- [x]`, `- [-]`) for task tracking

## Hidden Coupling Points

- **AgentEvent Clone**: `UserInputRequired` variant cannot be cloned due to `oneshot::Sender` - manual Clone impl converts to Progress
- **Patcher String Matching**: Exact string replacement required - whitespace differences cause failures
- **Plan File Parsing**: Markdown markers must follow exact format or parser fails silently
- **Session Store Schema**: SQLite schema versioning (`SCHEMA_VERSION`) - schema changes require migration
- **MCP Registry**: Server registry manages child process lifecycle - improper shutdown causes zombie processes

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
