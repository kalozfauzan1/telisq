//! Telisq Multi-Context Protocol (MCP) implementation.
//!
//! This module implements the core MCP server management, protocol handling,
//! and tool registry for Telisq. MCP enables communication between the Telisq
//! orchestrator and external servers (LLM agents) via JSON-RPC over stdio pipes.

pub mod protocol;
pub mod registry;
pub mod server;
pub mod tools;

pub use registry::McpRegistry;
pub use server::McpServer;
