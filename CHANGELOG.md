# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-04-01

### Added
- **Structured Planning** - Plan files with tasks, dependencies, and phases
- **Intelligent Execution** - AI-powered plan execution with orchestrator and agents
- **Real-time TUI** - Terminal UI for monitoring execution progress
- **Session Management** - List, resume, show, and delete sessions
- **Codebase Indexing** - Index project files for knowledge base
- **LLM Integration** - Support for OpenAI, Anthropic, and Ollama providers
- **MCP Support** - Model Context Protocol server integration
- **CLI Commands** - bootstrap, doctor, plan, run, status, index, session

### Fixed
- LLM client test failures (serde field naming mismatch)
- CLI argument conflicts in plan and run commands

### Notes
- This is the initial stable release
- Some features may have stub implementations
- Git repository not yet initialized
