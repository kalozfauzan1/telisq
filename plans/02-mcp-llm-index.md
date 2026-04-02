# Plan: MCP Runtime, LLM Layer, and Codebase Index
> Feature: implement integration primitives for tools, model calls, and semantic retrieval
> Created: 2026-04-01
> Status: completed

## Tasks

### [x] 1. Implement MCP JSON-RPC server process manager
**Files:** `mcp/src/lib.rs`, `mcp/src/server.rs`, `mcp/src/protocol.rs`
**Contract:**
- Spawn MCP servers from config command and args with stdio pipes.
- Execute initialize handshake using JSON-RPC 2.0 and protocol version from PRD.
- Support robust request ID generation and tool call request-response flow.
**Depends on:** —

---

### [x] 2. Implement MCP registry lifecycle and availability model
**Files:** `mcp/src/registry.rs`, `mcp/src/lib.rs`
**Contract:**
- Spawn all configured MCP servers at startup with non-fatal degrade behavior.
- Provide availability checks per server and call dispatch.
- Attempt one respawn on mid-run server failure, then degrade gracefully.
**Depends on:** 1

---

### [x] 3. Build agent-specific tool schemas
**Files:** `mcp/src/tools.rs`
**Contract:**
- Return correct tool sets for Plan, Code, Review, Ask, and Orchestrator.
- Gate optional tools by server availability.
- Enforce Code Agent file constraints via file tool implementations.
**Depends on:** 2

---

### [x] 4. Implement OpenAI-compatible LLM client and streaming
**Files:** `core/src/llm/mod.rs`, `core/src/llm/client.rs`, `core/src/llm/stream.rs`, `core/src/llm/tools.rs`
**Contract:**
- Implement non-streaming and streaming chat APIs with tool-call support.
- Handle malformed tool-call retries, rate-limit backoff, and context overflow strategy.
- Keep request format provider-neutral via configurable base_url.
**Depends on:** —

---

### [x] 5. Implement codebase index pipeline using Ollama and Qdrant
**Files:** `index/src/lib.rs`, `index/src/embedder.rs`, `index/src/store.rs`, `index/src/crawler.rs`, `index/src/watcher.rs`
**Contract:**
- Create per-project collection namespace and upsert chunk embeddings.
- Respect ignored directories and indexed extension policy.
- Support search query API and optional watch mode with debounce.
**Depends on:** —

---

### [x] 6. Add integration tests with mocked MCP and LLM
**Files:** `tests/integration/mcp_mock_test.rs`, `tests/integration/llm_mock_test.rs`, `tests/fixtures/*`
**Contract:**
- Validate MCP tool invocation and failure degrade behavior.
- Validate LLM tool-call serialization and retry logic.
- Ensure index search returns stable top-k shaped outputs with fixtures.
**Depends on:** 3, 4, 5

