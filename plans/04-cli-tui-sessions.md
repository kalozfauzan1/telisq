# Plan: CLI Commands, TUI, Session Management, and Bootstrap UX
> Feature: deliver operator experience and persistence paths
> Created: 2026-04-01
> Status: completed

## Tasks

### [x] 1. Build CLI command skeleton and argument wiring
**Files:** `cli/src/main.rs`, `cli/src/commands/mod.rs`, `cli/src/commands/plan.rs`, `cli/src/commands/run.rs`, `cli/src/commands/index.rs`, `cli/src/commands/status.rs`, `cli/src/commands/session.rs`, `cli/src/commands/doctor.rs`
**Contract:**
- Implement command set from PRD including plan, run, index, status, sessions, session resume, doctor, and bootstrap.
- Ensure options for continue, dry-run, and profile override are wired.
- Route commands to orchestrator and service layers without business logic duplication.
**Depends on:** —

---

### [x] 2. Implement first-run bootstrap and doctor diagnostics
**Files:** `cli/src/commands/doctor.rs`, `shared/src/config.rs`, `mcp/src/registry.rs`, `index/src/embedder.rs`, `index/src/store.rs`
**Contract:**
- Auto-create default config if missing and print actionable dependency checks.
- Implement `telisq doctor` checks for environment, services, MCP dry-run viability, and LLM endpoint connectivity.
- Add `telisq bootstrap` command to prepare local dependencies with pragmatic baseline behavior.
**Depends on:** 1

---

### [x] 3. Implement TUI app state and event loop
**Files:** `cli/src/tui/mod.rs`, `cli/src/tui/app.rs`, `cli/src/tui/events.rs`
**Contract:**
- Implement AppState fields and keyboard control flow from PRD.
- Merge keyboard and agent channel events via async select loop.
- Handle Ask Agent input mode and normal navigation mode safely.
**Depends on:** 1

---

### [x] 4. Implement TUI visual components and panels
**Files:** `cli/src/tui/components/index_bar.rs`, `cli/src/tui/components/sidebar.rs`, `cli/src/tui/components/plan_view.rs`, `cli/src/tui/components/session_view.rs`, `cli/src/tui/components/agent_panel.rs`
**Contract:**
- Render title, index health, sidebar navigation, plan details, session details, and agent activity.
- Display marker transitions and tool-call activity in near real-time.
- Support panel toggle and status-bar command hints.
**Depends on:** 3

---

### [x] 5. Implement session persistence and resume integration
**Files:** `shared/src/types.rs`, `core/src/orchestrator.rs`, `cli/src/commands/session.rs`, `cli/src/tui/app.rs`
**Contract:**
- Persist session snapshots as JSON and maintain SQLite index for listings.
- Implement resume by session ID and continue-from-plan flows.
- Restore orchestrator state and maintain session ID increment convention.
**Depends on:** 1, 3

---

### [x] 6. Implement UX safeguards for edge-case interactions
**Files:** `cli/src/tui/app.rs`, `core/src/orchestrator.rs`
**Contract:**
- Confirm quit during active run and ensure graceful interruption path.
- Handle missing editor fallback and plan parse errors during live run.
- Keep user-visible status aligned with orchestration events and errors.
**Depends on:** 4, 5
