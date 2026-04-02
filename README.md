# Telisq

Telisq is a structured planning and execution engine for software projects, designed to provide a reliable and efficient way to manage complex development workflows.

## Features

- **Structured Planning**: Create and manage detailed plans with dependencies and phases
- **Intelligent Execution**: Automate task execution with built-in agents (Plan, Code, Review, Ask)
- **Real-time Feedback**: Visualize progress and get instant feedback through the TUI
- **Session Management**: Save and resume work from any point with SQLite persistence
- **Codebase Indexing**: Semantic search and knowledge base for your project (Ollama + Qdrant)
- **LLM Integration**: Leverage AI models for planning and problem-solving with tool calling and SSE streaming
- **MCP Support**: Extend functionality with Multi-Context Protocol tools
- **Agent Orchestration**: DAG-based task execution with dependency ordering and error recovery

## Architecture

Telisq is organized as a Cargo workspace with 6 crates:

| Crate | Purpose |
|-------|---------|
| `shared/` | Core data types, errors, config, brief contracts |
| `plan/` | Plan parsing, validation, dependency graphs, marker tracking |
| `index/` | Codebase indexing (Ollama embeddings + Qdrant vector store) |
| `mcp/` | Multi-Context Protocol server implementation (JSON-RPC over stdio) |
| `core/` | Agent orchestration, LLM client, patcher, session store |
| `cli/` | CLI entry point + TUI (ratatui-based) |

## Installation

```bash
# Install from source
cargo install --path cli

# Or run directly
cargo run -p telisq-cli -- --help
```

## Getting Started

1. **Initialize a project**:
   ```bash
   telisq bootstrap
   ```

2. **Check your environment**:
   ```bash
   telisq doctor
   ```

3. **Create a plan**:
   ```bash
   telisq plan create
   ```

4. **Run your plan**:
   ```bash
   telisq run
   ```

5. **View progress**:
   ```bash
   telisq status
   ```

## Commands

### `telisq bootstrap`
Initialize a new Telisq project with default configuration.

### `telisq doctor`
Verify environment and dependencies:
- Rust toolchain version
- Node.js version
- `OPENAI_API_KEY` is set
- Ollama reachable and `nomic-embed-text` model available
- Qdrant reachable
- MCP servers availability
- LLM connectivity

### `telisq plan`
Manage plans with subcommands:
- `create` — Create a new plan (with optional `--goal` argument)
- `edit` — Edit an existing plan in `$EDITOR`
- `list` — List available plans
- `validate` — Validate a plan file

### `telisq run`
Execute the current plan with TUI:
- `--plan-path` — Path to plan file (auto-discovers in `plans/`)
- `--continue-from` — Resume from a specific task
- `--dry-run` — Dry run without making changes

### `telisq status`
Show plan progress, index health, MCP server availability, and LLM connectivity.

### `telisq index`
Manage codebase index with subcommands:
- `build` — Crawl project, embed chunks, upsert to Qdrant
- `search <query>` — Query Qdrant with user query, display top-k results
- `watch` — Start file watcher for live index updates
- `status` — Display index health

### `telisq session`
List and manage sessions with subcommands:
- `list` — Display sessions from SQLite with status
- `resume <id>` — Restore session and continue execution
- `show <id>` — Show session details with event history
- `delete <id>` — Mark session as canceled
- `export <id>` — Export session to JSON

## TUI Features

The terminal UI provides:
- **Plan View**: Real-time task markers ([ ], [~], [x], [!], [-])
- **Session View**: Task progress bar, recent events log, agent activity
- **Agent Panel**: Live agent message stream and status
- **Index Bar**: Index health and search results

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` | Quit (confirm if session active) |
| `Tab` | Switch active panel |
| `p` | Switch to Plan panel |
| `s` | Switch to Session panel |
| `a` | Switch to Agent panel |
| `i` | Switch to Index panel |
| `Space` | Pause/resume |
| `Enter` | Select task / confirm option |
| `Esc` | Cancel / go back |

## Configuration

Telisq reads configuration from:
1. `~/.telisq/config.yaml` (global config)
2. `.telisq.toml` (project-specific config)

### External Services

- **Ollama**: Embeddings at `http://localhost:11434` (model: `nomic-embed-text`)
- **Qdrant**: Vector storage at `http://localhost:6334`

## Development

### Prerequisites

- Rust 1.70 or later
- Node.js (for some tool integrations)
- Git
- Ollama running at `http://localhost:11434`
- Qdrant running at `http://localhost:6334`

### Build

```bash
cargo build
cargo build --release
```

### Testing

```bash
# Run all tests
cargo test

# Run specific crate tests
cargo test -p telisq-core
cargo test -p telisq-plan

# Run specific test file
cargo test --test orchestrator_test

# Run tests with logging
RUST_LOG=debug cargo test
```

### Code Quality

```bash
cargo clippy
cargo fmt
```

## Documentation

- [Release Checklist](docs/release-checklist.md)
- [Implementation Plans](plans/)
- [Architecture Overview](plans/00-implementation-master-plan.md)

## License

MIT
# telisq
