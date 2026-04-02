# Suggested Commands for Telisq Development

## Build Commands
- `cargo build` - Build all workspace crates
- `cargo build --release` - Build optimized release binary

## Test Commands
- `cargo test` - Run all tests (unit + integration)
- `cargo test --package telisq-core` - Run tests for specific crate
- `cargo test --test orchestrator_test` - Run single integration test
- `RUST_LOG=debug cargo test` - Run tests with debug logging

## Run Commands
- `cargo run --bin telisq -- plan` - Run planning phase
- `cargo run --bin telisq -- run` - Run execution phase with TUI
- `cargo run --bin telisq -- index` - Index codebase artifacts
- `cargo run --bin telisq -- doctor` - Run diagnostics
- `cargo run --bin telisq -- bootstrap` - Create default config
- `cargo run -p telisq-cli -- --help` - Show CLI help

## Lint and Format
- `cargo fmt` - Format code (rustfmt)
- `cargo clippy` - Run linter (clippy)
- `cargo clippy -- -D warnings` - Treat warnings as errors

## Debug Commands
- `RUST_LOG=debug cargo run --bin telisq -- run` - Enable debug logging
- `RUST_LOG=trace cargo run --bin telisq -- run` - Enable trace logging (most verbose)

## Git Commands
- `git status` - Check working tree status
- `git diff` - Show changes
- `git log --oneline -10` - Show recent commits