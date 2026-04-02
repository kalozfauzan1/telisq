# Project Debug Rules (Non-Obvious Only)

## Capability Boundary (Enforced)

- Scope: Reproduce issues, diagnose root cause, and apply minimal corrective changes.
- Allowed operations: diagnostic commands, targeted instrumentation, minimal bugfix edits.
- Required: remove temporary debug artifacts before completion.
- Forbidden operations: feature delivery or large refactors not required for the fix.

## Mandatory Tool Usage

- **Sequential Thinking**: Always use for systematic debugging
- **Context7**: Always query for:
  - Known issues with libraries in use
  - Debugging patterns for your framework/language

## Requirement Clarification Policy

- **ALWAYS ask before debugging** when:

  - Error context is incomplete
  - Expected vs. actual behavior is not clearly stated
  - Steps to reproduce are missing
  - Environment details are unclear

- **Use `ask_followup_question` tool** with:
  - Specific questions about the error context
  - 2-4 potential root causes to investigate

## Environment

- Requires Ollama running at `http://localhost:11434` for embeddings
- Requires Qdrant running at `http://localhost:6334` for vector storage
- SQLite database for session store (auto-created)
- Config at `~/.telisq/config.yaml` (global) or `.telisq.toml` (project override)

## Gotchas

- **MCP Protocol**: Uses JSON-RPC over stdio pipes (not HTTP) - cannot debug with browser dev tools
- **AgentEvent Cloning**: `AgentEvent::UserInputRequired` cannot be cloned due to `oneshot::Sender` - manual Clone impl converts it to Progress event
- **Patcher String Matching**: `Patcher::apply_patch()` uses exact string matching - whitespace differences cause silent failures
- **Plan File Markers**: Plan files use special markdown markers (`- [ ]`, `- [x]`, `- [-]`) - incorrect format breaks tracking
- **Test Fixtures**: Tests use `CARGO_MANIFEST_DIR` to locate fixtures - running tests from wrong directory causes failures
- **LLM Response Parsing**: LLM responses parsed with serde - malformed JSON causes parse errors, not API errors
- **SQLite Schema**: Session store uses schema versioning (`SCHEMA_VERSION` constant) - schema mismatches cause silent failures
- **Tracing Logs**: Uses `tracing` crate (not `log` or `println!`) - set `RUST_LOG` env var to see output
- **TUI Event Loop**: TUI uses `ratatui` with event loop in `cli/src/tui/events.rs` - blocking operations freeze UI

## Debugging Commands

- `RUST_LOG=debug cargo run --bin telisq -- run` - Enable debug logging
- `RUST_LOG=trace cargo run --bin telisq -- run` - Enable trace logging (most verbose)
- `cargo test --test orchestrator_test` - Run specific integration test
- `cargo test --package telisq-core` - Run core crate tests

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
