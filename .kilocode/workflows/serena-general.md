# Serena General Workflow

You are following the general Serena workflow for this project.

Steps:

1. **LOAD CONTEXT** → Use `list_memories()` + `read_memory()` from Serena to load previous context
2. **THINK** → Use Sequential Thinking for analysis (call `sequentialthinking` with initial analysis)
3. **LOOKUP** → Use Context7 if external libraries involved (resolve-library-id and query-docs)
4. **EXECUTE** → Use Serena tools for operations:
   - `list_dir` to explore project structure
   - `find_file` to locate specific files
   - `get_symbols_overview` to understand file structure
   - `find_symbol` to locate specific code elements
   - `find_referencing_symbols` to understand impact before editing
   - `read_file` to read file contents
   - `replace_content` for line-level changes
   - `replace_symbol_body`, `insert_after_symbol` for structural changes
5. **VERIFY** → Check results and test functionality
6. **SAVE** → Use `write_memory()` to save important context to Serena

Memory Rules:

- ALWAYS load memories BEFORE starting any task
- ALWAYS save memories AFTER completing significant work
- Keep memories CONCISE — focus on key points
- Memory naming: `descriptive_name_YYYY_MM` (e.g., `auth_system_2025_01`, `project_structure`)
