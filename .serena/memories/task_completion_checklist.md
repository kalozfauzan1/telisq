# Task Completion Checklist

When completing a task in Telisq, follow these steps:

## Before Completing
1. Run `cargo build` to ensure code compiles
2. Run `cargo test` to ensure all tests pass
3. Run `cargo clippy` to check for linting issues
4. Run `cargo fmt` to ensure code is formatted

## Plan Updates (if applicable)
1. Update the plan file checklist items to `- [x]`
2. Add timestamped progress log entry
3. Add short note of changed files
4. Set status to `Done` if implementation is complete
5. Add concise final summary

## Memory Management
1. Save relevant learnings to Serena memory using `write_memory()`
2. Memory naming: `descriptive_name_YYYY_MM`

## Commit Guidelines (if asked to commit)
1. Run `git status` to see changes
2. Run `git diff` to review changes
3. Run `git log --oneline -5` to see commit style
4. Create descriptive commit message
5. Do NOT commit unless explicitly asked

## Verification
- For new features: verify with tests
- For bug fixes: verify the fix resolves the issue
- For refactors: verify no behavior change