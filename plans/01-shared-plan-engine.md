# Plan: Shared Contracts and Plan Engine
> Feature: establish canonical types, config loading, plan parser, dependency graph, and marker tracker
> Created: 2026-04-01
> Status: completed

## Tasks

### [x] 1. Define all domain contracts in shared crate
**Files:** `shared/src/lib.rs`, `shared/src/types.rs`, `shared/src/brief.rs`, `shared/src/errors.rs`
**Contract:**
- Export canonical agent-facing contracts for `AgentBrief`, `AgentResult`, `TaskSpec`, `Session`, `TuiEvent`, and status enums.
- Ensure serde tags and snake_case mapping are consistent across all brief/result variants.
- Implement structured `AgentError` variants for LLM, MCP, parse, config, session, and file guard failures.
**Depends on:** —

---

### [x] 2. Implement global and per-project config loading
**Files:** `shared/src/config.rs`, `shared/src/lib.rs`
**Contract:**
- Load default config from `~/.telisq/config.yaml` with env interpolation support.
- Apply optional project override from `.telisq.toml`.
- Expose merged `AppConfig` with validated defaults for llm, index, agent, and mcp blocks.
**Depends on:** 1

---

### [x] 3. Build markdown plan parser with strict grammar and line-aware errors
**Files:** `plan/src/lib.rs`, `plan/src/parser.rs`
**Contract:**
- Parse plan files using PRD grammar including marker, files, contract bullets, and dependencies.
- Reject malformed structures with precise line numbers.
- Enforce sequential task IDs and unique task IDs.
**Depends on:** 1

---

### [x] 4. Implement dependency graph and validation
**Files:** `plan/src/graph.rs`, `plan/src/validator.rs`, `plan/src/lib.rs`
**Contract:**
- Validate all dependency references point to existing tasks.
- Detect cycles and return structured errors.
- Provide helper for resolving runnable tasks where dependencies are done.
**Depends on:** 3

---

### [x] 5. Implement atomic marker update engine
**Files:** `plan/src/tracker.rs`, `plan/src/lib.rs`
**Contract:**
- Replace status marker by task ID without mutating unrelated content.
- Perform atomic write via temp file then rename.
- Map orchestrator statuses to markers `[ ]`, `[~]`, `[x]`, `[!]`, `[-]`.
**Depends on:** 3

---

### [x] 6. Add shared and plan crate unit tests
**Files:** `tests/unit/config_test.rs`, `tests/unit/plan_parser_test.rs`, `tests/unit/plan_graph_test.rs`, `tests/unit/plan_tracker_test.rs`, `tests/fixtures/plans/*`
**Contract:**
- Cover valid parse, malformed format, invalid dependencies, cycle detection, sequential ID errors, and marker atomicity.
- Ensure config interpolation and override merge behavior are deterministic.
**Depends on:** 2, 4, 5

