# Project Structure Details

## Crate Dependencies
- `shared` - No internal dependencies, base types
- `plan` - Depends on `shared`
- `index` - Depends on `shared`
- `mcp` - Depends on `shared`
- `core` - Depends on `shared`, `plan`, `mcp`, `index`
- `cli` - Depends on `shared`, `plan`, `core`

## Key Files
- `cli/src/main.rs` - CLI entry point with clap subcommands
- `cli/src/tui/mod.rs` - TUI module
- `cli/src/tui/events.rs` - TUI event loop
- `core/src/orchestrator.rs` - Main orchestrator managing agent lifecycle
- `shared/src/brief.rs` - Agent brief contracts
- `shared/src/types.rs` - Core domain types
- `shared/src/errors.rs` - Shared error types
- `shared/src/config.rs` - Configuration models

## Test Structure
- `tests/unit/` - Unit tests (config_test, patcher_test, plan_tracker_test)
- `tests/integration/` - Integration tests (orchestrator_test, session_test, llm_mock_test, mcp_mock_test)
- `tests/fixtures/` - Test fixtures (plans/simple.plan.md, plans/dependency.plan.md)

## Plans Directory
- `plans/` - Implementation plans and task tracking
- Plan files use markdown with special markers for task status

## Configuration
- Global config: `~/.telisq/config.yaml`
- Project override: `.telisq.toml`
- Serena config: `.serena/project.yml`