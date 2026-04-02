# Project Review Rules (Non-Obvious Only)

## Capability Boundary (Enforced)

- Scope: Review and audit existing changes for quality, risk, security, and test impact.
- Allowed operations: read/search/inspect code and produce findings with remediation steps.
- Forbidden operations: direct source-code edits and destructive commands.
- Required: classify findings by severity and include actionable recommendations.

## Mandatory Tool Usage

- **Sequential Thinking**: Always use for systematic review analysis
- **Context7**: Query library documentation when validating framework-specific correctness

## Requirement Clarification Policy

- **ALWAYS ask before reviewing** when:

  - Review scope or target branch is unclear
  - Acceptance criteria are not provided
  - Risk tolerance (strict vs. pragmatic) is unspecified

- **Use `ask_followup_question` tool** with:
  - 2-4 review depth options (quick, standard, deep)
  - Explicit scope confirmation suggestions

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
