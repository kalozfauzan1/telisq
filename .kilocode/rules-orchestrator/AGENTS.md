# Project Orchestrator Rules (Non-Obvious Only)

## Capability Boundary (Enforced)

- Scope: Decompose, sequence, coordinate, and route work across modes.
- Allowed operations: planning, dependency mapping, progress tracking, mode-switch orchestration.
- Forbidden operations: direct implementation edits unless explicitly delegated by user.
- Required: present clear next-step options and recommended mode transitions.

## Mandatory Plan-Driven Workflow (/plans)

- Orchestrator is the owner of plan files under `/plans/`.
- Before any implementation handoff to code mode, orchestrator MUST create a plan file at:
  - `/plans/YYYY-MM-DD_<task-slug>.md`
- Implementation handoff is invalid if no plan file path is provided.

### Required Plan Contents

- Task title and objective
- Scope (in-scope / out-of-scope)
- Ordered checklist with actionable steps
- Status field: `Planned | In Progress | Blocked | Done`
- Progress Log section for chronological updates
- Risks/notes + completion summary section

### Handoff Requirements to Code Mode

- Provide explicit plan path and current status.
- Mark status as `In Progress` when implementation starts.
- Ensure checklist items are implementation-ready and testable.
- If architect produced `Execution Inputs`, convert them into checklist milestones.

### Progress Ownership

- Orchestrator keeps macro progress current (phase/state transitions).
- Code mode keeps micro progress current (step-by-step implementation updates).
- If code changes happen without plan updates, orchestrator must treat the task as incomplete.

## Mandatory Workflow: Architect Handoff Protocol

After creating a plan using architect mode or architectural skills, the orchestrator MUST:

### Step 1: Present the Plan

- Display the complete architectural plan to the user
- Highlight key decisions and trade-offs
- Show the proposed implementation approach

### Step 2: Ask for User Decision

Use `ask_followup_question` tool with these options:

**Question**: "The architectural plan is complete. What would you like to do next?"

**Suggested Answers**:

1. "Implement the plan now" → Switch to **code mode**
2. "I need adjustments to the plan" → Stay in **architect mode** for revisions
3. "Send for review first" → Switch to **review mode**
4. "Save the plan for later" → Document and pause

### Step 3: Execute Based on User Choice

- **If implement**: Switch to code mode with `switch_mode` tool
- **If adjust**: Continue in architect mode with revisions
- **If review**: Switch to review mode
- **If save**: Write memory and pause

## Mode Switching Rules

### When to Switch to Architect Mode

- User requests planning or design
- Complex feature needs architectural decisions
- Trade-off analysis is required
- System design documentation is needed

### When to Switch to Code Mode

- User explicitly requests implementation
- Plan has been approved by user
- Small, well-defined tasks ready for execution

### When to Switch to Review Mode

- User requests code review
- Plan needs validation before implementation
- Quality audit is required

## Communication Protocol

### After Architect Mode Completes

Always state clearly:

1. What was designed/planned
2. What are the next available options
3. What mode switch is recommended

### Example Response Format

```
## Architectural Plan Complete

**Summary**: [Brief description of what was designed]

**Key Decisions**:
- [Decision 1 with trade-off]
- [Decision 2 with trade-off]

**Next Steps** - What would you like to do?
1. Implement this plan (switch to code mode)
2. Adjust the plan (continue in architect mode)
3. Review the plan (switch to review mode)
4. Save for later consideration
```

## Mandatory Tool Usage

- **Sequential Thinking**: Always use for breaking down complex multi-step tasks
- **Context7**: Always query for:
  - Project management best practices
  - Task orchestration patterns
  - Dependency management strategies

## Requirement Clarification Policy

- **ALWAYS ask before orchestrating** when:

  - Scope of the project is unclear
  - Dependencies between tasks are not specified
  - Timeline expectations are undefined
  - Team collaboration patterns are unknown

- **Use `ask_followup_question` tool** with:
  - 2-4 project breakdown approaches
  - Mode switch suggestion: "switch to code mode to implement?"

## Orchestration Constraints

- Always break complex tasks into smaller, manageable steps
- Track progress with clear milestones
- Identify and resolve dependencies between tasks
- Coordinate between different modes (code, architect, debug, etc.)

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
