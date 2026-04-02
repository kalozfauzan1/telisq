# telisq — Product Requirements Document

> Derived from "telisik" (Indonesian) — to investigate, to examine carefully, to leave nothing assumed.
> telisq is a CLI coding agent that refuses to write a single line of code until it truly understands what you need.

**Version:** 2.0 (Production-Ready)
**Status:** Final Draft
**Tagline:** investigate before you execute

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Problem Statement](#2-problem-statement)
3. [Goals & Non-Goals](#3-goals--non-goals)
4. [Prerequisites & Environment](#4-prerequisites--environment)
5. [Technical Stack](#5-technical-stack)
6. [Multi-Agent Architecture](#6-multi-agent-architecture)
7. [Inter-Agent Communication Protocol](#7-inter-agent-communication-protocol)
8. [MCP Communication Protocol](#8-mcp-communication-protocol)
9. [Feature Requirements](#9-feature-requirements)
10. [Plan File Format & Parser Spec](#10-plan-file-format--parser-spec)
11. [TUI State Machine](#11-tui-state-machine)
12. [Error Handling & Edge Cases](#12-error-handling--edge-cases)
13. [Testing Strategy](#13-testing-strategy)
14. [CLI Commands Reference](#14-cli-commands-reference)
15. [Configuration Reference](#15-configuration-reference)
16. [Build Priority & Milestones](#16-build-priority--milestones)
17. [Suggestions for Optimization](#17-suggestions-for-optimization)
18. [Open Questions](#18-open-questions)
19. [Appendix](#19-appendix)

---

## 1. Executive Summary

telisq is an AI-powered coding agent CLI tool built in Rust, using a **multi-agent orchestration architecture**. A persistent Orchestrator session reasons about the overall goal and delegates to specialized sub-agents — Plan, Code, Review, and Ask — each running in a fully isolated sub-session.

**Core philosophy:** telisq investigates before it executes. The name comes from the Indonesian word "telisik" — to investigate, to examine carefully, to leave nothing assumed.

**Core problems solved:**

| # | Problem | Solution |
|---|---------|----------|
| P1 | Inconsistent plan placement | Always in `plans/` at project root |
| P2 | No progress tracking | Markers `[~]`/`[x]`/`[!]` updated in real-time |
| P3 | Repetitive MCP setup | Single `~/.telisq/config.yaml`, auto-loaded |
| P4 | Missing codebase context | Ollama embeddings + Qdrant vector search |

---

## 2. Problem Statement

| # | Pain Point | Impact |
|---|------------|--------|
| P1 | Inconsistent plan file location | Plans in random locations, impossible to track progress |
| P2 | No real-time progress updates | Agent works but never updates the plan |
| P3 | Repetitive MCP tool setup | Every session requires re-configuring MCP servers |
| P4 | No codebase indexing | Writes duplicates, misses existing patterns |

---

## 3. Goals & Non-Goals

### 3.1 Goals

- Multi-agent orchestration: Orchestrator delegates to Plan, Code, Review, Ask agents
- Consistent per-feature plan system in `plans/` directory
- Real-time plan progress tracking via marker updates
- Single global MCP config, loaded automatically
- Full codebase context via Ollama + Qdrant
- Claude Code-quality TUI with ratatui
- Any OpenAI-compatible LLM via configurable `base_url`
- Session management with full resume capability

### 3.2 Non-Goals (v1.0)

- GUI desktop application
- Cloud sync of plans or sessions
- True parallel multi-agent execution
- Built-in version control
- Non-OpenAI-compatible LLM APIs (Gemini native, etc.)

---

## 4. Prerequisites & Environment

### 4.1 Required Dependencies

Engineer must verify all of these are available before starting build:

```bash
# Rust toolchain
rustup toolchain install stable
rustup default stable
# Minimum version: 1.75.0 (for async traits stable support)
cargo --version  # must be >= 1.75.0

# Node.js (for MCP servers via npx)
node --version   # must be >= 18.0.0
npm --version    # must be >= 9.0.0

# Ollama (local embedding)
# Install: https://ollama.ai
ollama --version
ollama pull nomic-embed-text   # required embedding model
ollama pull qwen2.5-coder:7b   # optional: for local LLM

# Qdrant (vector database)
# Run via Docker:
docker run -d --name qdrant \
  -p 6333:6333 -p 6334:6334 \
  -v $(pwd)/qdrant_storage:/qdrant/storage \
  qdrant/qdrant:latest
# Verify: curl http://localhost:6334/healthz

# MCP servers (installed on first run via npx, no pre-install needed)
# context7, sequential-thinking, serena, bash
```

### 4.2 Environment Variables

```bash
# Required
OPENAI_API_KEY=sk-...          # or any OpenAI-compatible API key

# Optional overrides
TELISQ_CONFIG_PATH=~/.telisq/config.yaml   # default config location
TELISQ_LOG_LEVEL=info                       # trace|debug|info|warn|error
TELISQ_QDRANT_URL=http://localhost:6334     # override Qdrant URL
EDITOR=nvim                                 # used by 'e' keybinding in TUI
```

### 4.3 First-Run Bootstrap

On first `telisq` invocation, if config does not exist:

```
~/.telisq/config.yaml not found.
Creating default config... ✓

Checking dependencies:
  ✓ Ollama reachable (http://localhost:11434)
  ✓ nomic-embed-text model available
  ✓ Qdrant reachable (http://localhost:6334)
  ✗ OPENAI_API_KEY not set

Please set OPENAI_API_KEY in your environment or edit ~/.telisq/config.yaml
Run 'telisq doctor' to check all dependencies at any time.
```

### 4.4 `telisq doctor` Command

Checks all dependencies and prints status:

```
telisq doctor

Environment
  ✓ Rust 1.82.0
  ✓ Node.js 20.11.0
  ✓ OPENAI_API_KEY set

Services
  ✓ Ollama reachable — http://localhost:11434
  ✓ nomic-embed-text model available
  ✓ Qdrant reachable — http://localhost:6334

MCP Servers (tested via npx --yes dry-run)
  ✓ context7
  ✓ sequential-thinking
  ✓ serena
  ✓ bash

LLM Connectivity
  ✓ POST https://api.openai.com/v1/chat/completions → 200 OK
  Model: gpt-4o

All checks passed. telisq is ready.
```

---

## 5. Technical Stack

| Component | Technology | Version | Notes |
|-----------|-----------|---------|-------|
| Language | Rust | ≥ 1.75.0 | workspace, multi-crate |
| TUI | ratatui | 0.28 | Claude Code-style |
| Terminal backend | crossterm | 0.28 | cross-platform |
| LLM | OpenAI-compatible REST | — | configurable `base_url` |
| Embeddings | Ollama | latest | `nomic-embed-text` model |
| Vector DB | Qdrant | latest | semantic search |
| Session DB | SQLite via sqlx | 0.8 | per-project |
| MCP Runtime | Node.js / npx | ≥ 18.0 | stdio-based JSON-RPC |
| File ops | Surgical patch | — | read → diff → apply |
| Async runtime | tokio | 1 | full features |

### 5.1 Rust Workspace Layout

```
telisq/
├── Cargo.toml              # workspace root
├── cli/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # entry point
│       ├── commands/       # clap subcommands
│       │   ├── mod.rs
│       │   ├── plan.rs
│       │   ├── run.rs
│       │   ├── index.rs
│       │   ├── session.rs
│       │   └── doctor.rs
│       └── tui/            # ratatui components
│           ├── mod.rs
│           ├── app.rs      # AppState, event loop
│           ├── components/
│           │   ├── index_bar.rs
│           │   ├── sidebar.rs
│           │   ├── plan_view.rs
│           │   ├── session_view.rs
│           │   └── agent_panel.rs
│           └── events.rs
├── core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── orchestrator.rs  # Orchestrator agent loop
│       ├── agents/
│       │   ├── mod.rs
│       │   ├── plan_agent.rs
│       │   ├── code_agent.rs
│       │   ├── review_agent.rs
│       │   └── ask_agent.rs
│       ├── llm/
│       │   ├── mod.rs
│       │   ├── client.rs    # OpenAI-compatible HTTP client
│       │   ├── stream.rs    # SSE streaming
│       │   └── tools.rs     # tool call serialization
│       └── patcher.rs       # surgical file patcher
├── plan/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── parser.rs        # markdown plan parser
│       ├── tracker.rs       # marker updater
│       ├── validator.rs     # dependency validation
│       └── graph.rs         # dependency DAG
├── mcp/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── registry.rs      # server registry
│       ├── server.rs        # MCP server process manager
│       ├── protocol.rs      # JSON-RPC protocol
│       └── tools.rs         # tool dispatcher
├── shared/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs         # all domain types
│       ├── config.rs        # AppConfig + loader
│       ├── errors.rs        # AgentError enum
│       └── brief.rs         # AgentBrief + AgentResult
└── index/
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── embedder.rs      # Ollama HTTP client
        ├── store.rs         # Qdrant client
        ├── crawler.rs       # file system crawler
        └── watcher.rs       # notify file watcher
```

### 5.2 Complete Cargo.toml (workspace)

```toml
[workspace]
members = ["cli", "core", "plan", "mcp", "shared", "index"]
resolver = "2"

[workspace.dependencies]
# async
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
async-trait = "0.1"

# serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# error handling
anyhow = "1"
thiserror = "1"

# CLI + TUI
clap = { version = "4", features = ["derive", "color", "env"] }
ratatui = "0.28"
crossterm = "0.28"

# HTTP
reqwest = { version = "0.12", features = ["json", "stream"] }

# database
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "chrono", "uuid"] }

# utilities
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
dirs = "5"
notify = "6"
regex = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# diff / patch
similar = "2"

# process management
tokio-process = "1"
```

---

## 6. Multi-Agent Architecture

### 6.1 Overview

```
User
 │
 ▼
Orchestrator (persistent session — lives for duration of telisq run)
 │  Responsibilities: reason · decide · delegate · collect results
 │  Does NOT write code or files directly
 │
 ├─── Plan Agent    (isolated sub-session)  spawned once per feature
 ├─── Code Agent    (isolated sub-session)  spawned once per task
 ├─── Review Agent  (isolated sub-session)  spawned after all tasks [x]
 └─── Ask Agent     (isolated sub-session)  spawned on any uncertainty
```

**Isolation rule:** Sub-agents receive only their `AgentBrief`. They have no access to Orchestrator conversation history, other agent sessions, or global state. All communication is via structured `AgentBrief` → `AgentResult`.

### 6.2 Agent Type Definitions

#### Orchestrator

| Property | Value |
|----------|-------|
| Session type | Persistent — created on `telisq run`, lives until completion or quit |
| LLM model | `config.llm.model` (e.g. gpt-4o) |
| MCP tools | `sequential-thinking` only |
| Direct file access | Only `write_plan_marker()` — no code writing |
| Concurrency | Single-threaded decision loop; spawns one sub-agent at a time |

**Orchestrator tool definitions (OpenAI function calling format):**

```json
[
  {
    "type": "function",
    "function": {
      "name": "spawn_agent",
      "description": "Spawn a sub-agent with a specific brief and wait for its result",
      "parameters": {
        "type": "object",
        "properties": {
          "agent_type": {
            "type": "string",
            "enum": ["plan", "code", "review", "ask"]
          },
          "brief": {
            "type": "object",
            "description": "The AgentBrief payload. Structure depends on agent_type."
          }
        },
        "required": ["agent_type", "brief"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "write_plan_marker",
      "description": "Update the status marker of a task in the plan file",
      "parameters": {
        "type": "object",
        "properties": {
          "task_id": { "type": "integer" },
          "status": {
            "type": "string",
            "enum": ["todo", "in_progress", "done", "failed", "skipped"]
          }
        },
        "required": ["task_id", "status"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "read_plan",
      "description": "Read the current state of the plan file including all task statuses",
      "parameters": {
        "type": "object",
        "properties": {
          "plan_file": { "type": "string" }
        },
        "required": ["plan_file"]
      }
    }
  }
]
```

#### Plan Agent

| Property | Value |
|----------|-------|
| Session type | Sub-session, isolated |
| LLM model | `config.llm.model` |
| MCP tools | `context7`, `sequential-thinking`, codebase search (Qdrant) |
| Output | Creates `plans/<feature>.md` on disk |
| Returns | `PlanResult { plan_file, task_count, summary }` |

#### Code Agent

| Property | Value |
|----------|-------|
| Session type | Sub-session, isolated, one per task |
| LLM model | `config.llm.model` |
| MCP tools | `file_read`, `file_patch`, `file_write`, `bash`, codebase search, `context7`, `serena` |
| Scope | Only files listed in `CodeBrief.constraints.allowed_files` |
| Returns | `CodeResult { status, files_written, files_patched, attempts, errors, hypothesis }` |

#### Review Agent

| Property | Value |
|----------|-------|
| Session type | Sub-session, isolated |
| LLM model | `config.llm.model` |
| MCP tools | `bash`, `file_read`, codebase search |
| Trigger | After ALL tasks in plan are `[x]` |
| Returns | `ReviewResult { status, issues, test_results, summary }` |

#### Ask Agent

| Property | Value |
|----------|-------|
| Session type | Sub-session, isolated |
| LLM model | `config.llm.model_fast` (lightweight — just formats question) |
| MCP tools | None |
| Trigger | Any uncertainty: Code Agent 3x fail, Orchestrator ambiguity, Review issues requiring user decision |
| Returns | `AskResult { question, user_answer }` |

### 6.3 System Prompts

#### Orchestrator System Prompt

```
You are the Orchestrator for telisq — an AI coding agent.
telisq comes from "telisik" (Indonesian): investigate carefully, leave nothing assumed.

Your role: manage the full lifecycle of a feature implementation.
You reason, decide, and delegate. You NEVER write code yourself.

AVAILABLE TOOLS:
- spawn_agent(agent_type, brief) → AgentResult
- write_plan_marker(task_id, status)
- read_plan(plan_file)

PROCEDURE:
1. If plan file does not exist:
   → spawn Plan Agent to create it
   → ask user: run now / review first / open editor

2. For each task (in dependency order):
   a. Check: all depends_on tasks must be [x] before proceeding
   b. write_plan_marker(task_id, "in_progress")
   c. spawn Code Agent with task brief
   d. If Code Agent returns success → write_plan_marker(task_id, "done")
   e. If Code Agent returns failed:
      → spawn Ask Agent with failure context
      → Based on user answer:
        - "retry" → spawn new Code Agent with updated brief including user guidance
        - "skip"  → write_plan_marker(task_id, "skipped")
        - "stop"  → write_plan_marker(task_id, "failed"), halt execution

3. After all tasks are [x]:
   → spawn Review Agent
   → If issues found → spawn Code Agent for each error-severity issue
   → Spawn Review Agent again to verify fixes
   → Report completion summary to user

RULES:
- Always use sequential-thinking before making complex decisions
- Spawn Ask Agent for ANY uncertainty you cannot resolve alone
- Mark [!] only after user explicitly confirms "stop" via Ask Agent
- Save session state after each agent completes
- Communicate with user in Indonesian

You speak in Indonesian.
```

#### Plan Agent System Prompt

```
You are the Plan Agent for telisq.
Your job: generate a precise, executable plan file for a feature.

You must NOT generate the plan until all ambiguities are resolved.

PROCEDURE:
1. Scan codebase_context in your brief — pre-fill known answers
2. Detect library/framework names → call context7 immediately (auto)
3. Assess complexity:
   - If request touches >2 systems OR deps are non-obvious → call sequential-thinking
   - context7 and sequential-thinking can fire in parallel
4. Generate clarifying questions:
   - Max 5 per round, grouped by topic
   - Skip questions already answered by codebase scan
5. If answer is ambiguous → suggest: "Apakah maksud Anda X? Misalnya: [concrete example]"
6. Show full assumption summary — wait for ONE explicit confirmation
7. Generate plan file at plans/<feature-name>.md
8. Return { plan_file, task_count, summary }

PLAN FILE RULES:
- Tasks must be numbered sequentially starting from 1
- Each task must have: title, Files, Contract, Depends on
- Contract must be specific enough for a Code Agent to implement without asking questions
- Depends on must reference valid task numbers only

You speak in Indonesian.
```

#### Code Agent System Prompt

```
You are the Code Agent for telisq.
You implement exactly ONE task as specified in your brief.
You are fully isolated — you only have the context in your brief.

RULES:
- ONLY write to files listed in constraints.allowed_files
- Always call file_read on existing files before patching
- Use surgical patch — never overwrite unrelated code
- After every file write/patch, run the verify command via bash
- On verify failure: retry up to constraints.max_retries times
- On each retry: inject the full error output into your next attempt
- Use context7 if you need library documentation
- Use serena if you need to find existing symbols or references

RETURN when done:
- status: "success" if verify passed, "failed" if max retries exhausted
- files_written: list of new files created
- files_patched: list of existing files modified
- attempts: number of attempts made
- errors: all error messages encountered
- hypothesis: if failed, your best guess at the root cause

You speak in Indonesian in your reasoning.
```

#### Review Agent System Prompt

```
You are the Review Agent for telisq.
Your job: verify that a feature implementation is complete and correct.

PROCEDURE:
1. Read all files in files_changed from your brief
2. Run each command in verify_commands via bash
3. Check each task's contract in the plan file — verify it is fulfilled
4. Classify issues:
   - "error": must be fixed before feature is complete
   - "warning": should be fixed but not blocking
5. Return structured result

RETURN:
- status: "approved" if no errors, "issues_found" if any errors
- issues: list of { severity, file, line (if known), description }
- test_results: summary of test run output
- summary: 2-3 sentence human-readable summary

You speak in Indonesian.
```

#### Ask Agent System Prompt

```
You are the Ask Agent for telisq.
Your job: present a specific question to the user and return their answer.

RULES:
- Be concise — explain context in 1-2 sentences maximum
- Present options clearly if provided in your brief
- Accept free text answer if allow_free_text is true
- Do not editorialize or suggest what the user should choose
- Return the user's answer verbatim in user_answer field

You speak in Indonesian.
```

### 6.4 Full Orchestration Flow

```
telisq run plans/login-auth.md
│
├── [startup]
│   ├── Load ~/.telisq/config.yaml
│   ├── Spawn MCP servers (all from config, keep alive)
│   ├── Check Qdrant connectivity — warn if unreachable
│   ├── Check .telisq/sessions/ for prior sessions on this plan
│   └── If sessions exist:
│       → show prompt: [c] resume last / [n] new session / [v] view history
│
├── [plan phase — if plan file does not exist]
│   ├── Orchestrator calls spawn_agent("plan", PlanBrief)
│   ├── Plan Agent conducts telisik phase interactively
│   ├── Plan Agent writes plans/<feature>.md
│   ├── Plan Agent returns PlanResult
│   └── Orchestrator asks user: [y] run now / [n] review first / [e] open editor
│       If [e]: open $EDITOR, wait for close, re-parse plan, ask again
│
├── [execution loop]
│   LOOP until all tasks [x] or halt:
│   │
│   ├── Orchestrator calls read_plan()
│   ├── Find next task: status=todo AND all depends_on are done
│   │   If no such task AND tasks remain → deadlock → spawn Ask Agent
│   │
│   ├── Orchestrator calls write_plan_marker(task_id, "in_progress")
│   │   (writes [~] to plan file on disk immediately)
│   │
│   ├── Orchestrator calls spawn_agent("code", CodeBrief)
│   │   Code Agent runs in isolated context
│   │   Code Agent returns CodeResult
│   │
│   ├── If CodeResult.status == "success":
│   │   └── write_plan_marker(task_id, "done") → next iteration
│   │
│   └── If CodeResult.status == "failed":
│       ├── spawn_agent("ask", AskBrief with failure context)
│       ├── If user_answer == "retry":
│       │   └── spawn new Code Agent with brief augmented with user guidance
│       ├── If user_answer == "skip":
│       │   └── write_plan_marker(task_id, "skipped") → next iteration
│       └── If user_answer == "stop":
│           └── write_plan_marker(task_id, "failed") → HALT
│
├── [review phase — after all tasks done]
│   ├── Orchestrator calls spawn_agent("review", ReviewBrief)
│   ├── If ReviewResult.status == "approved":
│   │   └── Report completion → DONE
│   └── If ReviewResult.status == "issues_found":
│       ├── For each issue with severity "error":
│       │   └── spawn_agent("code", CodeBrief for fix)
│       └── spawn_agent("review", ReviewBrief) again to verify fixes
│
└── [completion]
    ├── Save full session to .telisq/sessions/
    └── Print summary: tasks completed, files changed, review status
```

---

## 7. Inter-Agent Communication Protocol

### 7.1 Overview

Sub-agents are **not separate processes or threads**. They are implemented as async Rust functions within the same process, each with their own isolated `Vec<Message>` conversation history. The Orchestrator calls `spawn_agent()` which is implemented as:

```rust
// core/src/orchestrator.rs
async fn spawn_agent(
    agent_type: AgentType,
    brief: AgentBrief,
    config: &AppConfig,
    mcp_registry: &McpRegistry,
    index: &CodebaseIndex,
    event_tx: &Sender<TuiEvent>,  // for live TUI updates
) -> Result<AgentResult, AgentError>
```

This function:
1. Constructs a fresh `Vec<Message>` with only the system prompt + brief as user message
2. Runs the agent's LLM loop until it returns a structured result
3. Sends progress events to `event_tx` for TUI updates
4. Returns `AgentResult` to Orchestrator

### 7.2 AgentBrief Type Definitions

```rust
// shared/src/brief.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "agent_type", rename_all = "snake_case")]
pub enum AgentBrief {
    Plan(PlanBrief),
    Code(CodeBrief),
    Review(ReviewBrief),
    Ask(AskBrief),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanBrief {
    pub feature_request: String,
    pub codebase_context: Vec<CodeSnippet>,  // top_k from Qdrant
    pub plan_directory: PathBuf,             // always "plans/"
    pub max_clarification_rounds: u32,       // from config, default 5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBrief {
    pub task: TaskSpec,
    pub plan_section: String,                // raw markdown of this task
    pub completed_tasks_summary: String,     // "Task 1: X done. Task 2: Y done."
    pub codebase_context: Vec<CodeSnippet>,  // top_k from Qdrant for this task
    pub constraints: CodeConstraints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: u32,
    pub title: String,
    pub files: Vec<PathBuf>,
    pub contract: String,
    pub depends_on: Vec<u32>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeConstraints {
    pub max_retries: u32,
    pub patch_strategy: PatchStrategy,
    pub allowed_files: Vec<PathBuf>,   // MUST match task.files exactly
    pub verify_command: String,        // e.g. "cargo check" or "tsc --noEmit"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewBrief {
    pub feature: String,
    pub plan_file: PathBuf,
    pub files_changed: Vec<PathBuf>,
    pub verify_commands: Vec<String>,  // e.g. ["cargo test", "cargo clippy"]
    pub fail_on_warnings: bool,        // default false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskBrief {
    pub context: String,               // why are we asking
    pub question: String,              // the specific question
    pub options: Vec<String>,          // suggested options (may be empty)
    pub allow_free_text: bool,         // default true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub file: PathBuf,
    pub snippet: String,
    pub relevance_score: f32,
}
```

### 7.3 AgentResult Type Definitions

```rust
// shared/src/brief.rs (continued)

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "agent_type", rename_all = "snake_case")]
pub enum AgentResult {
    Plan(PlanResult),
    Code(CodeResult),
    Review(ReviewResult),
    Ask(AskResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanResult {
    pub plan_file: PathBuf,
    pub task_count: u32,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeResult {
    pub status: AgentStatus,
    pub task_id: u32,
    pub files_written: Vec<PathBuf>,
    pub files_patched: Vec<PathBuf>,
    pub attempts: u32,
    pub errors: Vec<String>,
    pub hypothesis: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub status: ReviewStatus,
    pub issues: Vec<ReviewIssue>,
    pub test_results: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    pub severity: IssueSeverity,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskResult {
    pub question: String,
    pub user_answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus { Success, Failed }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus { Approved, IssuesFound }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity { Error, Warning }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchStrategy { Surgical, Overwrite }
```

### 7.4 TUI Event Channel

The agent runner communicates with TUI via a `tokio::sync::mpsc` channel:

```rust
// shared/src/types.rs

#[derive(Debug, Clone)]
pub enum TuiEvent {
    // Agent lifecycle
    AgentSpawned { agent_type: AgentType, task_id: Option<u32> },
    AgentCompleted { agent_type: AgentType, task_id: Option<u32>, status: AgentStatus },

    // Progress updates (for live agent activity panel)
    AgentLog { agent_type: AgentType, message: String },
    ToolCallStarted { tool_name: String, args_preview: String },
    ToolCallCompleted { tool_name: String, result_preview: String },

    // Plan file changes
    PlanMarkerUpdated { task_id: u32, status: TaskStatus },

    // Index updates
    IndexProgress { indexed: u32, total: u32 },
    IndexComplete { file_count: u32 },

    // User interaction required (Ask Agent)
    UserInputRequired { question: String, options: Vec<String> },

    // Session
    SessionSaved { session_id: String },

    // Errors
    Error { message: String, recoverable: bool },
}
```

---

## 8. MCP Communication Protocol

### 8.1 Overview

MCP servers communicate via **JSON-RPC 2.0 over stdio**. telisq spawns each MCP server as a child process using `tokio::process::Command` and communicates via `stdin`/`stdout` pipes.

### 8.2 Server Lifecycle

```rust
// mcp/src/server.rs

pub struct McpServerProcess {
    pub name: String,
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    request_id: AtomicU64,
}

impl McpServerProcess {
    pub async fn spawn(server_config: &McpServer) -> Result<Self> {
        // spawn: npx -y @upstash/context7-mcp
        let mut child = Command::new(&server_config.command)
            .args(&server_config.args)
            .envs(&server_config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())  // suppress MCP server logs
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut server = Self { name: server_config.name.clone(), process: child, stdin, stdout, request_id: AtomicU64::new(1) };

        // MCP initialize handshake
        server.initialize().await?;
        Ok(server)
    }

    async fn initialize(&mut self) -> Result<()> {
        // Send MCP initialize request (JSON-RPC 2.0)
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "telisq", "version": "1.0.0" }
            }
        });
        self.send(request).await?;
        let response = self.recv().await?;
        // validate response, store server capabilities
        Ok(())
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        self.send(request).await?;
        let response = self.recv().await?;

        if let Some(error) = response.get("error") {
            return Err(AgentError::Mcp(format!("MCP error: {}", error)));
        }
        Ok(response["result"].clone())
    }

    async fn send(&mut self, msg: Value) -> Result<()> {
        let line = serde_json::to_string(&msg)? + "\n";
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Value> {
        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;
        Ok(serde_json::from_str(&line)?)
    }
}
```

### 8.3 MCP Registry

```rust
// mcp/src/registry.rs

pub struct McpRegistry {
    servers: HashMap<String, McpServerProcess>,
}

impl McpRegistry {
    pub async fn spawn_all(config: &McpConfig) -> Result<Self> {
        let mut servers = HashMap::new();
        for server_config in &config.servers {
            match McpServerProcess::spawn(server_config).await {
                Ok(process) => { servers.insert(server_config.name.clone(), process); }
                Err(e) => {
                    tracing::warn!("Failed to spawn MCP server '{}': {}", server_config.name, e);
                    // Non-fatal: warn and continue without this server
                }
            }
        }
        Ok(Self { servers })
    }

    pub async fn call(&mut self, server: &str, tool: &str, args: Value) -> Result<Value> {
        let server = self.servers.get_mut(server)
            .ok_or_else(|| AgentError::Mcp(format!("MCP server '{}' not available", server)))?;
        server.call_tool(tool, args).await
    }

    pub fn is_available(&self, server: &str) -> bool {
        self.servers.contains_key(server)
    }
}
```

### 8.4 Tool Definitions for LLM

These are injected into every agent's LLM call as the `tools` parameter:

```rust
// mcp/src/tools.rs

pub fn get_tools_for_agent(agent_type: &AgentType, registry: &McpRegistry) -> Vec<Value> {
    let mut tools = vec![];

    match agent_type {
        AgentType::Code => {
            tools.push(file_read_tool());
            tools.push(file_write_tool());
            tools.push(file_patch_tool());
            tools.push(bash_tool());
            tools.push(codebase_search_tool());
            if registry.is_available("context7") { tools.push(context7_tool()); }
            if registry.is_available("serena") { tools.push(serena_tool()); }
        }
        AgentType::Plan => {
            tools.push(codebase_search_tool());
            if registry.is_available("context7") { tools.push(context7_tool()); }
            if registry.is_available("sequential-thinking") { tools.push(seq_thinking_tool()); }
        }
        AgentType::Review => {
            tools.push(file_read_tool());
            tools.push(bash_tool());
            tools.push(codebase_search_tool());
        }
        AgentType::Ask => {
            // No tools — Ask Agent only formats questions
        }
        AgentType::Orchestrator => {
            tools.push(spawn_agent_tool());
            tools.push(write_plan_marker_tool());
            tools.push(read_plan_tool());
            if registry.is_available("sequential-thinking") { tools.push(seq_thinking_tool()); }
        }
    }
    tools
}
```

---

## 9. Feature Requirements

### 9.1 LLM Client

```rust
// core/src/llm/client.rs

pub struct LlmClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl LlmClient {
    // Non-streaming: for agents that need full response before acting
    pub async fn chat(&self, model: &str, messages: &[Message], tools: &[Value], max_tokens: u32) -> Result<LlmResponse>

    // Streaming: for displaying live output to user (Ask Agent, Plan Agent clarification)
    pub async fn chat_stream(&self, model: &str, messages: &[Message], tools: &[Value], max_tokens: u32) -> Result<impl Stream<Item = Result<StreamChunk>>>
}

pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCallRequest>,
    pub finish_reason: FinishReason,
    pub usage: TokenUsage,
}

pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

**OpenAI-compatible request format:**

```json
{
  "model": "gpt-4o",
  "messages": [...],
  "tools": [...],
  "tool_choice": "auto",
  "max_tokens": 8192,
  "temperature": 0.2,
  "stream": false
}
```

### 9.2 Surgical File Patcher

```rust
// core/src/patcher.rs

pub struct SurgicalPatcher;

impl SurgicalPatcher {
    /// Read existing file, apply patch, write back
    pub async fn patch(file: &Path, patch: &str) -> Result<PatchResult> {
        let original = fs::read_to_string(file).await
            .unwrap_or_default();  // empty string if file doesn't exist

        // LLM provides patch as unified diff OR full replacement of specific sections
        // telisq supports two patch formats:
        // 1. Full file content (if file is new or small)
        // 2. Unified diff format (--- original, +++ modified)
        let patched = apply_patch(&original, patch)?;

        // Compute diff for TUI display
        let diff = compute_diff(&original, &patched);

        fs::write(file, &patched).await?;
        Ok(PatchResult { diff, lines_added: diff.insertions, lines_removed: diff.deletions })
    }
}

pub struct PatchResult {
    pub diff: String,        // unified diff for TUI display
    pub lines_added: u32,
    pub lines_removed: u32,
}
```

### 9.3 Codebase Indexer

```rust
// index/src/lib.rs

pub struct CodebaseIndex {
    embedder: OllamaEmbedder,
    store: QdrantStore,
    project_root: PathBuf,
    collection_name: String,  // sha256 of absolute project path
}

impl CodebaseIndex {
    pub async fn new(project_root: &Path, config: &IndexConfig) -> Result<Self> {
        // collection_name = "telisq_" + hex(sha256(project_root.to_string_lossy()))
        // This ensures per-project namespace in Qdrant
    }

    pub async fn index_all(&self, tx: Sender<TuiEvent>) -> Result<()> {
        // Crawl project, skip: .git, target, node_modules, .telisq
        // For each file: embed → upsert to Qdrant
        // Send IndexProgress events via tx
    }

    pub async fn search(&self, query: &str, top_k: usize) -> Result<Vec<CodeSnippet>> {
        // Embed query → search Qdrant → return top_k results
    }

    pub fn is_ready(&self) -> bool {
        // returns true if collection exists and has points
    }
}
```

**Ignored paths during indexing:**

```rust
const IGNORED_DIRS: &[&str] = &[
    ".git", "target", "node_modules", ".telisq",
    "dist", "build", ".next", "__pycache__", ".venv",
];

const INDEXED_EXTENSIONS: &[&str] = &[
    "rs", "toml", "ts", "tsx", "js", "jsx", "py", "go",
    "md", "yaml", "yml", "json", "sql", "sh", "env",
];
```

### 9.4 Session Storage

```rust
// shared/src/types.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,                          // "{plan-name}-{NNN}" e.g. "login-auth-003"
    pub plan_file: PathBuf,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub stopped_at_task: Option<u32>,
    pub orchestrator_history: Vec<Message>,  // full Orchestrator conversation
    pub sub_agent_log: Vec<SubAgentLogEntry>,
    pub files_written: Vec<PathBuf>,
    pub files_patched: Vec<PathBuf>,
    pub token_usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentLogEntry {
    pub agent_type: AgentType,
    pub task_id: Option<u32>,
    pub spawned_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub status: AgentStatus,
    pub files_changed: Vec<PathBuf>,
    pub conversation: Vec<Message>,          // sub-agent's isolated history
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus { Active, Paused, Completed, Failed }
```

**Storage:**
- Sessions stored as JSON files: `.telisq/sessions/{session-id}.json`
- SQLite index at `.telisq/sessions/sessions.db` for fast listing
- Session ID format: `{kebab-plan-name}-{zero-padded-3-digit-number}` e.g. `login-auth-003`

---

## 10. Plan File Format & Parser Spec

### 10.1 Formal Grammar (EBNF)

```ebnf
plan_file       ::= header NEWLINE+ overview NEWLINE+ "---" NEWLINE+ "## Tasks" NEWLINE+ task+
header          ::= "# Plan: " title NEWLINE metadata+
metadata        ::= "> " key ": " value NEWLINE
title           ::= [^\n]+
key             ::= "Feature" | "Created" | "Status"
value           ::= [^\n]+

overview        ::= "## Overview" NEWLINE paragraph+
paragraph       ::= [^\n]+ NEWLINE

task            ::= task_header NEWLINE+ field+ "---" NEWLINE*
task_header     ::= "### " marker " " task_id ". " task_title NEWLINE
marker          ::= "[ ]" | "[~]" | "[x]" | "[!]" | "[-]"
task_id         ::= [0-9]+
task_title      ::= [^\n]+

field           ::= files_field | contract_field | depends_field | notes_field
files_field     ::= "**Files:**" SPACE file_list NEWLINE
file_list       ::= backtick_path ("," SPACE backtick_path)*
backtick_path   ::= "`" filepath "`"
filepath        ::= [^`]+

contract_field  ::= "**Contract:**" NEWLINE contract_item+
contract_item   ::= "- " [^\n]+ NEWLINE

depends_field   ::= "**Depends on:**" SPACE depends_value NEWLINE
depends_value   ::= "—" | task_id_list
task_id_list    ::= task_id (", " task_id)*

notes_field     ::= "**Notes:**" SPACE [^\n]+ NEWLINE
```

### 10.2 Parser Implementation Notes

```rust
// plan/src/parser.rs

impl PlanParser {
    pub fn parse(content: &str) -> Result<Plan, ParseError> {
        // Returns ParseError with line number on malformed input
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Line {line}: {message}")]
    InvalidFormat { line: u32, message: String },

    #[error("Task {id} depends on task {dep} which does not exist")]
    InvalidDependency { id: u32, dep: u32 },

    #[error("Duplicate task ID: {id}")]
    DuplicateTaskId { id: u32 },

    #[error("Task IDs must be sequential starting from 1, found gap at {id}")]
    NonSequentialId { id: u32 },
}
```

### 10.3 Marker Update (Atomic Write)

```rust
// plan/src/tracker.rs

impl PlanTracker {
    /// Update task marker atomically — write to temp file then rename
    pub async fn update_marker(plan_file: &Path, task_id: u32, status: TaskStatus) -> Result<()> {
        let content = fs::read_to_string(plan_file).await?;
        let updated = Self::replace_marker(&content, task_id, status)?;

        // Atomic write: write to .tmp then rename
        let tmp = plan_file.with_extension("md.tmp");
        fs::write(&tmp, &updated).await?;
        fs::rename(&tmp, plan_file).await?;
        Ok(())
    }
}
```

### 10.4 Status Markers

| Marker | Meaning | Set by |
|--------|---------|--------|
| `[ ]` | Todo | Plan Agent or user (manual) |
| `[~]` | In progress | Orchestrator (on task start) |
| `[x]` | Done | Orchestrator (on Code Agent success) |
| `[!]` | Failed | Orchestrator (user confirmed stop) |
| `[-]` | Skipped | Orchestrator (user confirmed skip) |

### 10.5 Example Plan File

```markdown
# Plan: Login Auth
> Feature: implement login auth with JWT
> Created: 2025-01-15
> Status: in_progress

## Overview
JWT-based authentication with refresh token support.
Stack: Axum + SQLx + PostgreSQL + argon2 + jsonwebtoken.

---

## Tasks

### [x] 1. Setup User model
**Files:** `src/models/user.rs`
**Contract:**
- Struct `User`: `id: Uuid`, `email: String`, `password_hash: String`, `created_at: DateTime<Utc>`
- `User::new(email: &str, password: &str) -> Result<User>`
- `User::verify_password(&self, password: &str) -> bool`
**Depends on:** —

---

### [~] 2. JWT token service
**Files:** `src/services/auth.rs`, `src/middleware/auth.rs`
**Contract:**
- `generate_token(user_id: Uuid, secret: &str) -> Result<String>` — 15min expiry
- `validate_token(token: &str, secret: &str) -> Result<Claims>`
- `AuthMiddleware` — Axum extractor, rejects unauthorized with 401
**Depends on:** 1

---

### [ ] 3. Login & register routes
**Files:** `src/routes/auth.rs`
**Contract:**
- `POST /auth/register` → `201 { id, email }`
- `POST /auth/login` → `200 { token, refresh_token }`
**Depends on:** 2

---
```

---

## 11. TUI State Machine

### 11.1 AppState

```rust
// cli/src/tui/app.rs

pub struct AppState {
    // Navigation
    pub active_tab: Tab,               // Plans | Sessions
    pub selected_plan_idx: usize,
    pub selected_session_idx: usize,
    pub selected_task_idx: usize,
    pub show_agent_panel: bool,        // toggled by 'a'

    // Data
    pub plans: Vec<PlanSummary>,       // loaded from plans/ directory
    pub sessions: Vec<SessionSummary>, // loaded from .telisq/sessions/
    pub current_plan: Option<Plan>,
    pub current_session: Option<Session>,

    // Agent state
    pub agent_running: bool,
    pub agent_paused: bool,
    pub current_agent_type: Option<AgentType>,
    pub agent_log: Vec<AgentLogEntry>,  // for activity panel
    pub index_status: IndexStatus,

    // User input (for Ask Agent)
    pub awaiting_user_input: Option<UserInputPrompt>,
    pub user_input_buffer: String,

    // Event channel from agent runner
    pub event_rx: Receiver<TuiEvent>,

    // Error display
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tab { Plans, Sessions }

#[derive(Debug, Clone)]
pub struct IndexStatus {
    pub state: IndexState,
    pub indexed_files: u32,
    pub total_files: u32,
    pub last_indexed: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndexState { NotIndexed, Indexing, Ready }

#[derive(Debug, Clone)]
pub struct UserInputPrompt {
    pub question: String,
    pub options: Vec<String>,
    pub allow_free_text: bool,
}
```

### 11.2 Event Loop

```rust
// cli/src/tui/app.rs

impl AppState {
    pub async fn run(mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            // 1. Draw current state
            terminal.draw(|frame| self.render(frame))?;

            // 2. Poll for events (keyboard OR agent events) with timeout
            tokio::select! {
                // Keyboard events (16ms timeout = ~60fps)
                maybe_event = wait_for_key_event(Duration::from_millis(16)) => {
                    if let Some(event) = maybe_event? {
                        if self.handle_key_event(event).await? == ControlFlow::Break {
                            return Ok(());
                        }
                    }
                }
                // Agent events (non-blocking)
                maybe_tui_event = self.event_rx.recv() => {
                    if let Some(event) = maybe_tui_event {
                        self.handle_tui_event(event).await?;
                    }
                }
            }
        }
    }
}
```

### 11.3 Key Event Handling

```rust
impl AppState {
    async fn handle_key_event(&mut self, event: KeyEvent) -> Result<ControlFlow<()>> {
        // If awaiting user input (Ask Agent), route all keys to input buffer
        if self.awaiting_user_input.is_some() {
            return self.handle_input_key(event).await;
        }

        match event.code {
            KeyCode::Char('q') => return Ok(ControlFlow::Break),
            KeyCode::Char('r') => self.start_run().await?,
            KeyCode::Char('d') => self.start_dry_run().await?,
            KeyCode::Char('p') => self.toggle_pause().await?,
            KeyCode::Char('c') if self.agent_paused => self.resume().await?,
            KeyCode::Char('e') => self.open_editor().await?,
            KeyCode::Char('i') => self.trigger_reindex().await?,
            KeyCode::Char('a') => self.show_agent_panel = !self.show_agent_panel,
            KeyCode::Tab => self.active_tab = self.active_tab.toggle(),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::Enter => self.handle_enter().await?,
            KeyCode::Esc => self.handle_escape(),
            _ => {}
        }
        Ok(ControlFlow::Continue(()))
    }
}
```

### 11.4 Rendering Layout

```rust
// cli/src/tui/app.rs

impl AppState {
    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Vertical split: titlebar(1) + index_bar(2) + body(rest) + statusbar(1)
        let layout = Layout::vertical([
            Constraint::Length(1),   // titlebar
            Constraint::Length(2),   // index bar
            Constraint::Fill(1),     // body
            Constraint::Length(1),   // status bar
        ]).split(area);

        self.render_titlebar(frame, layout[0]);
        self.render_index_bar(frame, layout[1]);
        self.render_body(frame, layout[2]);
        self.render_statusbar(frame, layout[3]);
    }

    fn render_body(&self, frame: &mut Frame, area: Rect) {
        // Horizontal split: sidebar(21) + main panel(rest)
        // If agent panel shown: main splits into plan_view + agent_panel
        let sidebar_width = 21;

        let h_layout = if self.show_agent_panel {
            Layout::horizontal([
                Constraint::Length(sidebar_width),
                Constraint::Percentage(50),
                Constraint::Fill(1),
            ]).split(area)
        } else {
            Layout::horizontal([
                Constraint::Length(sidebar_width),
                Constraint::Fill(1),
            ]).split(area)
        };

        self.render_sidebar(frame, h_layout[0]);
        self.render_main_panel(frame, h_layout[1]);
        if self.show_agent_panel {
            self.render_agent_panel(frame, h_layout[2]);
        }
    }
}
```

---

## 12. Error Handling & Edge Cases

### 12.1 Error Types

```rust
// shared/src/errors.rs

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    Llm(String),

    #[error("LLM rate limit — retrying in {retry_after}s")]
    RateLimit { retry_after: u64 },

    #[error("LLM context overflow — conversation history too long")]
    ContextOverflow,

    #[error("MCP server '{server}' error: {message}")]
    Mcp { server: String, message: String },

    #[error("MCP server '{server}' not available")]
    McpUnavailable { server: String },

    #[error("Qdrant error: {0}")]
    Qdrant(String),

    #[error("Ollama error: {0}")]
    Ollama(String),

    #[error("Plan parse error: {0}")]
    PlanParse(#[from] ParseError),

    #[error("File not allowed: {path} is not in allowed_files for this task")]
    FileNotAllowed { path: PathBuf },

    #[error("Session error: {0}")]
    Session(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),
}
```

### 12.2 Edge Case Behaviors

| Scenario | Behavior |
|----------|----------|
| Qdrant unreachable on startup | Warn in TUI index bar: "index unavailable — running without codebase context". Continue without crashing. |
| Ollama unreachable on startup | Same as above — warn, continue. Index commands will fail with clear error. |
| MCP server fails to spawn | Log warning, mark server unavailable. Agent continues with available tools. |
| MCP server crashes mid-execution | Catch error, attempt respawn once. If respawn fails, continue without that tool. |
| Plan file manually edited during run | Parser re-reads plan on each `read_plan()` call. If parse fails, spawn Ask Agent: "Plan file has format error on line N. Stop execution?" |
| Plan file dependency cycle (A→B, B→A) | Detected at parse time by `plan::graph` cycle detection. Return `ParseError::CyclicDependency`. |
| All remaining tasks are blocked (deadlock) | Orchestrator detects no runnable tasks exist. Spawn Ask Agent: "Tasks [4,5,6] are blocked and their dependencies are not complete. How would you like to proceed?" |
| LLM returns malformed tool call JSON | Retry the LLM call with error message appended: "Your previous response had a malformed tool call. Please try again." Max 3 retries, then propagate error. |
| LLM rate limit (429) | Exponential backoff: wait `retry_after` seconds (from response header), max wait 60s, max 5 retries. Show countdown in TUI. |
| LLM context overflow (token limit) | Summarize oldest 50% of conversation history, continue. Log warning to agent panel. |
| Code Agent writes to non-allowed file | Block at `file_write`/`file_patch` tool implementation level. Return error: "File not in allowed_files for this task." Code Agent sees this as a tool error and must choose a different approach. |
| `$EDITOR` not set | Default to `nano` on Linux/macOS, `notepad` on Windows. If neither available, print: "Set $EDITOR env var to your preferred editor." |
| Qdrant collection not found | Create collection automatically on first index. |
| Session DB not found | Create `.telisq/sessions/` directory and `sessions.db` automatically on first run. |
| User presses `q` while agent running | Show confirmation: "Agent is running. Quit anyway? (y/N)". If yes, gracefully stop after current tool call completes, save session. |
| Plan file does not exist on `telisq run` | Orchestrator asks user: "No plan found at plans/login-auth.md. Create one now? (Y/n)". If yes, spawn Plan Agent. |

### 12.3 Graceful Shutdown

```rust
// core/src/orchestrator.rs

impl Orchestrator {
    pub async fn shutdown(&mut self) {
        // 1. Set shutdown flag — checked after each tool call
        self.shutdown_requested.store(true, Ordering::SeqCst);
        // 2. Wait for current tool call to complete (max 10s)
        // 3. Save session state
        // 4. Update plan markers: [~] → [ ] (reset in-progress tasks)
        // 5. Send shutdown signal to all MCP servers
    }
}
```

---

## 13. Testing Strategy

### 13.1 Test Structure

```
tests/
├── unit/
│   ├── plan_parser_test.rs     # parser with fixture files
│   ├── plan_tracker_test.rs    # marker update, atomic write
│   ├── plan_graph_test.rs      # dependency DAG, cycle detection
│   ├── patcher_test.rs         # surgical patch correctness
│   └── config_test.rs          # config loading, env var interpolation
├── integration/
│   ├── llm_mock_test.rs        # agent loop with mock LLM
│   ├── mcp_mock_test.rs        # MCP protocol with mock server
│   ├── orchestrator_test.rs    # full orchestration with mocked agents
│   └── session_test.rs         # session save/resume
└── fixtures/
    ├── plans/                  # sample plan files (valid + invalid)
    └── projects/               # minimal project structures for testing
```

### 13.2 LLM Mock

```rust
// tests/integration/llm_mock_test.rs

/// MockLlmClient allows tests to pre-program LLM responses
/// without making real API calls
pub struct MockLlmClient {
    responses: VecDeque<LlmResponse>,
}

impl MockLlmClient {
    pub fn new(responses: Vec<LlmResponse>) -> Self {
        Self { responses: responses.into() }
    }

    pub fn respond_with_tool_call(tool: &str, args: Value) -> LlmResponse {
        LlmResponse {
            content: None,
            tool_calls: vec![ToolCallRequest {
                id: "call_001".to_string(),
                name: tool.to_string(),
                arguments: args,
            }],
            finish_reason: FinishReason::ToolCalls,
            usage: TokenUsage::default(),
        }
    }
}
```

### 13.3 Key Test Cases

```rust
// Unit tests — plan parser
#[test] fn test_parse_valid_plan()
#[test] fn test_parse_missing_files_field()        // → ParseError
#[test] fn test_parse_invalid_dependency_ref()     // → ParseError::InvalidDependency
#[test] fn test_parse_cyclic_dependency()          // → ParseError::CyclicDependency
#[test] fn test_marker_update_preserves_content()  // patch only marker, not rest of file
#[test] fn test_marker_update_atomic()             // no partial writes visible

// Unit tests — surgical patcher
#[test] fn test_patch_new_file()
#[test] fn test_patch_existing_file_adds_function()
#[test] fn test_patch_does_not_touch_unrelated_code()

// Integration tests — orchestrator
#[test] async fn test_orchestrator_runs_tasks_in_dependency_order()
#[test] async fn test_orchestrator_blocks_on_failed_dependency()
#[test] async fn test_orchestrator_spawns_ask_agent_on_code_failure()
#[test] async fn test_orchestrator_spawns_review_agent_after_all_done()
#[test] async fn test_orchestrator_handles_deadlock()

// Integration tests — session
#[test] async fn test_session_saved_after_agent_completes()
#[test] async fn test_session_resume_loads_orchestrator_history()
#[test] async fn test_session_id_increments_per_plan()
```

### 13.4 Running Tests

```bash
# Unit tests only (no external services needed)
cargo test --workspace --lib

# Integration tests (requires: mock servers only, no real LLM/Qdrant)
cargo test --workspace --test '*'

# Full integration with real services (requires OPENAI_API_KEY + Qdrant + Ollama)
TELISQ_TEST_REAL=1 cargo test --workspace --test '*'

# Test specific crate
cargo test -p plan

# With logging
RUST_LOG=debug cargo test 2>&1 | less
```

---

## 14. CLI Commands Reference

| Command | Description |
|---------|-------------|
| `telisq` | Open interactive TUI (default) |
| `telisq plan "<feature>"` | Spawn Plan Agent, conduct telisik phase, generate plan |
| `telisq run plans/<f>.md` | Start Orchestrator, execute plan, new session |
| `telisq run plans/<f>.md --continue` | Resume from last session |
| `telisq run plans/<f>.md --dry-run` | Preview operations without writing |
| `telisq run plans/<f>.md --profile <n>` | Override MCP profile |
| `telisq index .` | Index codebase into Qdrant |
| `telisq index . --watch` | Index then watch for changes |
| `telisq status plans/<f>.md` | Print progress (no TUI) |
| `telisq sessions` | List all sessions for current project |
| `telisq session resume <id>` | Resume a specific session |
| `telisq doctor` | Check all dependencies and connectivity |

---

## 15. Configuration Reference

```yaml
# ~/.telisq/config.yaml

llm:
  base_url: https://api.openai.com/v1   # any OpenAI-compatible endpoint
  api_key: ${OPENAI_API_KEY}            # env var reference supported
  model: gpt-4o                         # Orchestrator + Code + Review agents
  model_fast: gpt-4o-mini              # Ask Agent (lightweight interactions)
  max_tokens: 8192
  temperature: 0.2
  timeout_seconds: 120                  # HTTP timeout per request

index:
  embedding_model: nomic-embed-text
  qdrant_url: http://localhost:6334
  top_k: 8
  auto_reindex: true
  debounce_seconds: 5                   # wait N seconds after last file change before reindex

agent:
  max_retries: 3
  patch_strategy: surgical              # surgical | overwrite
  verify_command: "cargo check"         # default verify command (overridden per-project)
  notify: false                         # OS notification on completion

mcp:
  servers:
    - name: context7
      command: npx
      args: ["-y", "@upstash/context7-mcp"]

    - name: sequential-thinking
      command: npx
      args: ["-y", "@modelcontextprotocol/server-sequential-thinking"]

    - name: serena
      command: npx
      args: ["-y", "serena-mcp"]

    - name: bash
      command: npx
      args: ["-y", "@modelcontextprotocol/server-bash"]
```

### 15.1 Per-Project Config (optional)

```toml
# <project-root>/.telisq.toml
# Overrides global config for this project only

[agent]
verify_command = "cargo check --all-targets"

[index]
top_k = 12
```

### 15.2 Provider Examples

```yaml
# Groq
base_url: https://api.groq.com/openai/v1
model: llama-3.3-70b-versatile

# Ollama (fully local — no API key needed)
base_url: http://localhost:11434/v1
api_key: ollama
model: qwen2.5-coder:32b

# OpenRouter
base_url: https://openrouter.ai/api/v1
model: anthropic/claude-sonnet-4-5

# LM Studio
base_url: http://localhost:1234/v1
api_key: lm-studio
model: local-model
```

---

## 16. Build Priority & Milestones

| Sprint | Milestone | Deliverables | Crates | Done when |
|--------|-----------|-------------|--------|-----------|
| 1 | Shared Foundation | All types in `shared`: AgentBrief, AgentResult, TaskSpec, Session, TuiEvent, AgentError. Config loader with env var interpolation. | `shared` | `cargo test -p shared` passes |
| 2 | Plan Engine | Parser with formal grammar, marker updater (atomic), dependency graph, cycle detection, validator. | `plan` | All parser unit tests pass including error cases |
| 3 | MCP Registry | JSON-RPC stdio protocol, server spawner, registry, tool definitions for all agent types. | `mcp` | `telisq doctor` shows all MCP servers ✓ |
| 4 | LLM Client | OpenAI-compatible HTTP client, streaming, tool call serialization, rate limit handling, context overflow handling. | `core/llm` | Mock LLM integration tests pass |
| 5 | Agent Runners | Plan Agent, Code Agent, Review Agent, Ask Agent, Orchestrator with full flow. Surgical patcher. | `core` | Orchestrator integration tests pass with mock LLM |
| 6 | CLI skeleton | clap commands, `telisq doctor`, basic stdout output (no TUI yet). | `cli` | `telisq run` works end-to-end in terminal |
| 7 | Codebase Index | Ollama embedder, Qdrant client, crawler, file watcher with debounce, namespace per project. | `index` | `telisq index .` indexes a Rust project correctly |
| 8 | TUI | ratatui layout, AppState, event loop, all panels, agent activity panel, keybindings. | `cli/tui` | Full TUI with live agent updates |
| 9 | Session Management | SQLite storage, session save/resume, session list/detail in TUI. | `core` + `cli` | Resume session restores full context |
| 10 | Polish | `telisq doctor`, error messages, graceful shutdown, per-project `.telisq.toml`. | all | `telisq doctor` passes on clean machine |

---

## 17. Suggestions for Optimization

> Items marked `💡 SUGGESTION` are not confirmed requirements.

### 17.1 Planning

#### 💡 SUGGESTION — Plan complexity score
Compute score from task count × system count × dependency depth.
Display: `"Complexity: Medium (6 tasks, 3 systems)"`.
Orchestrator auto-adjusts token budget per task.

#### 💡 SUGGESTION — Plan templates
`telisq plan --template rest-endpoint "..."`.
Pre-built task structures for: REST endpoint, auth module, DB migration, React component.
Plan Agent still runs telisik phase but starts with validated structure.

### 17.2 Execution

#### 💡 SUGGESTION — Token budget per Code Agent
`CodeBrief.constraints.max_tokens` (default 8192).
At 80% consumed: Code Agent summarizes context and compresses before continuing.

#### 💡 SUGGESTION — Test-aware Code Agent
Auto-detect test files for target module.
Run tests after each file write. Treat test failures same as compile errors.

#### 💡 SUGGESTION — Rollback on user stop
Track pre-task file snapshots for all `allowed_files`.
On "stop" via Ask Agent: offer to restore original file contents.

### 17.3 Sessions

#### 💡 SUGGESTION — Session branching
`telisq session branch <id> --from-task 3`.
Orchestrator loads history up to task 3, spawns fresh Code Agent with different strategy.

#### 💡 SUGGESTION — Session export
`telisq session export <id> --format md`.
Generate PR description / handoff summary from session log.

### 17.4 UX

#### 💡 SUGGESTION — Live diff in agent panel
Show unified diff of current file being patched in agent activity panel.
Toggle with `f`.

#### 💡 SUGGESTION — Cost estimation
Before run: estimate tokens per agent × number of tasks × model pricing.
`"Estimated: ~42k tokens (~$0.08)"`. Show actual usage in session summary.

#### 💡 SUGGESTION — OS notification
`agent.notify: true` → system notification when agent completes or needs input.

### 17.5 Infrastructure

#### 💡 SUGGESTION — Docker Compose bootstrap
`telisq bootstrap` → pulls and starts Qdrant + Ollama with correct models.
Reduces setup time from ~30 minutes to ~2 minutes.

#### 💡 SUGGESTION — Orchestrator uses stronger model
Separate config: `llm.model_orchestrator` (e.g. o1 or claude-opus).
Orchestrator does the most complex reasoning; Code Agent does most of the token volume.
Allows cost optimization: smart orchestrator + fast code agent.

---

## 18. Open Questions

1. **Serena LSP config** — How is the target language detected and which LSP binary is used? Is this auto-detected from project files (`Cargo.toml` → rust-analyzer, `package.json` → typescript-language-server)?

2. **Qdrant + Ollama startup** — Should `telisq bootstrap` auto-start services via `docker compose`, or document manual setup only for v1.0?

3. **Binary distribution** — `cargo install telisq` (simplest), Homebrew tap, or GitHub releases with pre-built binaries?

4. **`verify_command` auto-detection** — Should telisq auto-detect the verify command from project type (`Cargo.toml` → `cargo check`, `package.json` → `npm run build`)? Or require explicit config?

5. **MCP version pinning** — Should `config.yaml` support `args: ["-y", "context7@1.2.3"]` to pin versions and prevent breaking changes?

6. **Orchestrator model** — Should Orchestrator default to a different (more capable) model than Code Agent, or use the same `config.llm.model` for both?

7. **Plan Agent interactivity** — During the telisik clarification phase, the Plan Agent needs to display questions and receive answers in real-time. Implementation: stream Plan Agent output to stdout/TUI and read user input from stdin. Exact implementation of this interactive loop needs to be defined.

---

## 19. Appendix

### Appendix A — Project File Structure

```
my-project/
├── .telisq/
│   ├── sessions/
│   │   ├── sessions.db
│   │   ├── login-auth-001.json
│   │   └── login-auth-002.json
│   └── .telisq.toml              # optional per-project overrides
├── plans/
│   ├── login-auth.md
│   ├── user-profile.md
│   └── payment-integration.md
└── src/
    └── ...
```

### Appendix B — Global Config Structure

```
~/.telisq/
└── config.yaml
```

### Appendix C — Qdrant Collection Schema

```json
{
  "collection_name": "telisq_a3f2c1d4...",
  "vectors": {
    "size": 768,
    "distance": "Cosine"
  },
  "payload_schema": {
    "file_path": "string",
    "file_extension": "string",
    "chunk_index": "integer",
    "content": "string",
    "last_modified": "datetime"
  }
}
```

### Appendix D — Session ID Convention

```
Format: {kebab-plan-name}-{NNN}
Examples:
  login-auth-001
  payment-integration-003
  user-profile-001

NNN is zero-padded, increments per plan file.
Stored in sessions.db: SELECT MAX(id) WHERE plan = 'login-auth'
```

---

*telisq PRD v2.0 — investigate before you execute*
