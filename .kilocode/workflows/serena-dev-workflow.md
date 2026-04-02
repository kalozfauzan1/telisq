# Serena Development Workflow

You are following the Serena development workflow for this project.

Steps:

1. **LOAD CONTEXT** → Use `list_memories()` + `read_memory()` from Serena to load previous context
2. **THINK** → Use Sequential Thinking for analysis (call `sequentialthinking` with initial analysis)
3. **LOOKUP** → Use Context7 if external libraries involved (resolve-library-id and query-docs)
4. **EXECUTE** → Use Serena tools for code operations:
   - `find_symbol` to locate code elements
   - `get_symbols_overview` to understand file structure
   - `find_referencing_symbols` to understand impact before editing
   - `replace_symbol_body`, `insert_after_symbol`, `replace_content` for editing
5. **VERIFY** → Check results and test functionality
6. **SAVE** → Use `write_memory()` to save important context to Serena

Memory Rules:

- ALWAYS load memories BEFORE starting any task
- ALWAYS save memories AFTER completing significant work
- Keep memories CONCISE — focus on key points
- Memory naming: `descriptive_name_YYYY_MM` (e.g., `auth_system_2025_01`)
