//! OpenAI-compatible LLM client implementation.
//!
//! This module provides a generic OpenAI-compatible LLM client with support for
//! chat completions, tool calls, and multi-turn conversations.

pub mod client;
pub mod stream;
pub mod tools;
pub mod types;

pub use client::LlmClient;
pub use stream::{stream_chat_completion, StreamChunk};
