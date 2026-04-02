# Plan: Testing Matrix, Hardening, and Release Readiness
> Feature: ensure reliability, observability, and production-ready release path
> Created: 2026-04-01
> Status: completed

## Tasks

### [x] 1. Implement complete test harness structure
**Files:** `tests/unit/plan_parser_test.rs`, `tests/unit/plan_tracker_test.rs`, `tests/unit/plan_graph_test.rs`, `tests/unit/patcher_test.rs`, `tests/unit/config_test.rs`, `tests/integration/orchestrator_test.rs`, `tests/integration/session_test.rs`, `tests/integration/llm_mock_test.rs`, `tests/integration/mcp_mock_test.rs`
**Contract:**
- Mirror PRD test taxonomy and include deterministic fixture coverage.
- Validate parser, marker atomicity, patch behavior, orchestrator ordering, and session resume semantics.
- Ensure mock-based tests avoid external service dependencies by default.
**Depends on:** —

---

### [x] 2. Add real-service integration gate for optional confidence runs
**Files:** `tests/integration/*`, `README.md`
**Contract:**
- Introduce environment-gated test path for real LLM, Qdrant, and Ollama checks.
- Keep default CI path fast and deterministic while enabling deep validation locally.
- Document required environment and commands clearly.
**Depends on:** 1

---

### [x] 3. Harden error handling and degradation behavior
**Files:** `shared/src/errors.rs`, `core/src/orchestrator.rs`, `mcp/src/registry.rs`, `core/src/llm/client.rs`, `cli/src/tui/app.rs`
**Contract:**
- Validate all PRD edge-case behaviors including degraded mode, retries, and deadlock resolution prompts.
- Ensure errors remain user-actionable and non-destructive.
- Confirm marker consistency on interruption and failed task paths.
**Depends on:** 1

---

### [x] 4. Add instrumentation and logs for debugability
**Files:** `cli/src/main.rs`, `core/src/orchestrator.rs`, `mcp/src/server.rs`, `index/src/lib.rs`
**Contract:**
- Configure structured tracing with environment-controlled log levels.
- Emit key lifecycle logs for startup, tool calls, retries, session saves, and shutdown.
- Keep sensitive material out of logs by default.
**Depends on:** 3

---

### [x] 5. Validate command-level done criteria and milestone closure
**Files:** `README.md`, `docs/release-checklist.md`
**Contract:**
- Verify milestone acceptance for all ten PRD build priorities.
- Confirm command behavior for `doctor`, `bootstrap`, `plan`, `run`, `status`, `index`, `sessions`, and `session resume`.
- Produce release checklist with explicit pass-fail items.
**Depends on:** 2, 3, 4

---

### [x] 6. Prepare implementation handoff packet for code mode
**Files:** `plans/00-implementation-master-plan.md`, `plans/01-shared-plan-engine.md`, `plans/02-mcp-llm-index.md`, `plans/03-agents-orchestrator.md`, `plans/04-cli-tui-sessions.md`, `plans/05-testing-release.md`
**Contract:**
- Ensure all plan files use executable task language with dependencies.
- Confirm pragmatic baseline decisions are encoded consistently.
- Mark initial execution order so implementation can start without re-planning.
**Depends on:** 5
