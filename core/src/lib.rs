//! Telisq core engine.
//!
//! This module implements the core Telisq functionality including:
//! - OpenAI-compatible LLM client
//! - LLM request/response handling
//! - Agent orchestration
//! - Index management
//! - Session persistence

pub mod agents;
pub mod llm;
pub mod orchestrator;
pub mod patcher;
pub mod session;

pub use agents::AgentRunner;
pub use llm::LlmClient;
pub use orchestrator::Orchestrator;
pub use patcher::Patcher;
pub use session::SessionStore;
