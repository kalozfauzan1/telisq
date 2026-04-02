# Telisq v1 Release Checklist

## Pre-Release Requirements
- [ ] All dependencies are up to date and compatible
- [ ] Cargo.lock is committed and consistent with Cargo.toml
- [ ] All git status is clean (no uncommitted changes)

## Build and Installation
- [ ] `cargo build -p telisq-cli` completes successfully
- [ ] `cargo build --release -p telisq-cli` completes successfully
- [ ] `cargo run -p telisq-cli -- --help` shows help
- [ ] `cargo install --path cli` installs the `telisq` binary

## Tests
- [ ] All unit tests pass: `cargo test`
- [ ] All integration tests pass: `cargo test --test integration`
- [ ] Test coverage meets minimum requirements
- [ ] No flaky tests in the default deterministic suite

## Command Functionality
- [ ] `telisq doctor` - Verifies system dependencies and configuration
  - [ ] Checks for required tools (git, etc.)
  - [ ] Verifies LLM API connectivity (if configured)
  - [ ] Checks for valid configuration file

- [ ] `telisq bootstrap` - Initializes a new Telisq project
  - [ ] Creates .telisq directory structure
  - [ ] Generates default configuration
  - [ ] Initializes git repository

- [ ] `telisq plan` - Manages planning phases
  - [ ] Creates new plan files
  - [ ] Validates existing plans
  - [ ] Edits plan files
  - [ ] Lists current plans

- [ ] `telisq run` - Executes the plan
  - [ ] Runs plan execution in interactive mode
  - [ ] Handles task dependencies correctly
  - [ ] Tracks progress and updates plan markers

- [ ] `telisq status` - Displays plan progress
  - [ ] Shows current phase and task
  - [ ] Displays completion percentages
  - [ ] Lists completed and pending tasks

- [ ] `telisq index` - Manages the knowledge base index
  - [ ] Indexes project files
  - [ ] Searches the index
  - [ ] Removes outdated entries

- [ ] `telisq session` - Manages sessions
  - [ ] Lists available sessions
  - [ ] Creates new sessions
  - [ ] Removes old sessions

- [ ] `telisq session resume` - Resumes a saved session
  - [ ] Loads session state correctly
  - [ ] Resumes plan execution from where it left off
  - [ ] Handles session locking

## Error Handling and Hardening
- [ ] Application recovers from LLM API failures
- [ ] Degraded mode works correctly when services are unavailable
- [ ] Retry mechanism works for transient errors
- [ ] Deadlock resolution prompts are displayed correctly
- [ ] Plan markers remain consistent on interruption/failure
- [ ] User-facing errors are actionable and non-destructive

## Documentation
- [ ] README.md is up to date with latest features
- [ ] Command-line help is accurate
- [ ] Configuration options are documented
- [ ] Examples and tutorials are available
- [ ] Environment variables for testing are documented

## Performance
- [ ] Startup time is acceptable
- [ ] Task execution time is reasonable
- [ ] Memory usage is within acceptable limits
- [ ] No significant leaks detected

## Security
- [ ] No sensitive data is logged
- [ ] Configuration files are properly secured
- [ ] API keys and secrets are handled correctly

## Compatibility
- [ ] Works on all supported platforms (Windows, macOS, Linux)
- [ ] Compatibility with major LLM providers (OpenAI, Anthropic, Ollama)
- [ ] Works with different MCP server implementations

## Release Process
- [ ] Version number is updated in Cargo.toml files
- [ ] CHANGELOG.md is updated with release notes
- [ ] Git tag is created
- [ ] Release artifacts are uploaded to appropriate channels

## Post-Release Verification
- [ ] Install and run from released artifacts
- [ ] Verify all key features work as expected
- [ ] Monitor error rates and user feedback
