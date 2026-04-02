# Project Documentation Rules (Non-Obvious Only)

## Ask Mode Specific Rules

## Capability Boundary (Enforced)

- Scope: Explanation, analysis, and requirement clarification only.
- Allowed operations: read/search/inspect operations and question-asking.
- Forbidden operations: source edits, file create/delete, destructive commands, dependency changes.
- Handoff trigger: If implementation is requested, switch to Code mode before any file change.

### Mandatory Tool Usage

- **Sequential Thinking**: Always use before answering questions
- **Context7**: Always query for library documentation

### Requirement Clarification Policy

- **ALWAYS ask before proceeding** when requirements are ambiguous or vague
- **Use `ask_followup_question` tool** with 2-4 suggested answers
- **Never assume** - explicitly state assumptions and ask for confirmation

### Non-Obvious Project Context

- **Workspace Structure**: 6 crates (shared, plan, index, mcp, core, cli) - not a single-crate project
- **CLI Entry Point**: `cli/src/main.rs` uses `clap` for subcommands (plan, run, index, status, session, doctor, bootstrap)
- **TUI Location**: Terminal UI is in `cli/src/tui/` using `ratatui` library - not a web interface
- **Agent Types**: 4 agents (Plan, Code, Review, Ask) defined in `shared::brief::AgentType`
- **Plan Files**: Markdown files in `/plans/` directory with special task markers format
- **Config Location**: Global config at `~/.telisq/config.yaml`, project override at `.telisq.toml`
- **MCP Servers**: External LLM agents spawned as child processes communicating via stdio (not HTTP)
- **Index System**: Uses Ollama (embeddings) + Qdrant (vector store) - separate from main application
- **Session Store**: SQLite database via `sqlx` - not in-memory or file-based JSON
- **Test Structure**: Unit tests in `tests/unit/`, integration tests in `tests/integration/`, fixtures in `tests/fixtures/`

### Key Documentation Files

- `README.md` - Project overview and setup instructions
- `telisq-PRD.md` - Product requirements document (detailed spec)
- `plans/` - Implementation plans with task breakdowns
- `CHANGELOG.md` - Version history

### Serena Workflow (Mandatory)

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
