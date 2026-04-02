# Plan: Agent Runners and Orchestrator Loop
> Feature: implement isolated sub-agent execution and orchestrator lifecycle
> Created: 2026-04-01
> Status: completed

## Tasks

### [x] 1. Implement shared agent runtime interface
**Files:** `core/src/agents/mod.rs`, `core/src/lib.rs`
**Contract:**
- Define a common runner contract for Plan, Code, Review, and Ask agents.
- Standardize message history isolation and result marshaling to `AgentResult`.
- Emit progress events to TUI channel from agent loops.
**Depends on:** —

---

### [x] 2. Implement Plan Agent clarification and plan generation flow
**Files:** `core/src/agents/plan_agent.rs`, `core/src/llm/stream.rs`
**Contract:**
- Run clarification rounds with max bound and ambiguity guard.
- Use codebase context and optional MCP tools for evidence gathering.
- Write plan files into `plans/` and return summary with task count.
**Depends on:** 1

---

### [x] 3. Implement Code Agent single-task execution with retries
**Files:** `core/src/agents/code_agent.rs`, `core/src/patcher.rs`
**Contract:**
- Enforce allowed file constraints from brief.
- Apply surgical patch strategy and run verify command after each write.
- Retry with error feedback up to max_retries and report hypothesis on failure.
- Include test-aware behavior by running relevant tests for target module when available.
**Depends on:** 1

---

### [x] 4. Implement Review Agent verification workflow
**Files:** `core/src/agents/review_agent.rs`
**Contract:**
- Verify task contracts against files changed and run verify command list.
- Classify issues into blocking error and non-blocking warning.
- Return structured approval or issues_found response.
**Depends on:** 1

---

### [x] 5. Implement Ask Agent user-decision bridge
**Files:** `core/src/agents/ask_agent.rs`
**Contract:**
- Render concise context and options without editorializing.
- Return user answer verbatim for orchestrator decisions.
- Support both guided option selection and free text flow.
**Depends on:** 1

---

### [x] 6. Implement Orchestrator loop and dependency-safe execution
**Files:** `core/src/orchestrator.rs`
**Contract:**
- Implement full procedure: plan creation path, task loop, marker transitions, review pass, and completion summary.
- Guarantee dependency order execution and deadlock handling.
- Handle Code Agent failure via Ask Agent decision route: retry, skip, stop.
- Add stronger model configuration path for Orchestrator reasoning.
**Depends on:** 2, 3, 4, 5

---

### [x] 7. Implement graceful shutdown and session-safe interruption
**Files:** `core/src/orchestrator.rs`, `shared/src/types.rs`
**Contract:**
- Finish in-flight tool call, persist session, and reset in-progress markers on shutdown.
- Broadcast relevant events to TUI and session storage layer.
**Depends on:** 6

