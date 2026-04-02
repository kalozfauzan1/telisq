// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shared::brief::AgentType;
use shared::types::{SessionId, TaskId};
use std::fmt::Debug;
use tokio::sync::oneshot;
use uuid::Uuid;

/// Unique identifier for an agent instance.
pub type AgentId = String;

/// Unique identifier for a sub-session.
pub type SubSessionId = Uuid;

/// Result type for agent execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentResult {
    /// Agent completed successfully with a result.
    Success(serde_json::Value),
    /// Agent failed to complete.
    Failure(String),
    /// Agent requires user input.
    UserInputRequired(String),
}

/// User option for Ask Agent display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUserOption {
    /// Unique identifier for the option.
    pub id: String,
    /// Display text for the option.
    pub text: String,
    /// Whether the option allows free text input.
    pub allow_free_text: bool,
}

/// Event types for agent progress tracking.
/// Note: Cannot derive Serialize/Deserialize due to oneshot::Sender in UserInputRequired.
pub enum AgentEvent {
    /// Agent has started execution.
    Started,
    /// Agent has made progress.
    Progress(String),
    /// Agent has completed execution.
    Completed(AgentResult),
    /// Agent requires user input (Ask Agent specific).
    UserInputRequired {
        /// The question to display to the user.
        question: String,
        /// Available options for the user.
        options: Vec<AgentUserOption>,
        /// Channel for receiving the user's answer.
        answer_tx: oneshot::Sender<String>,
    },
}

// Manual Debug impl for AgentEvent due to oneshot::Sender
impl Debug for AgentEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentEvent::Started => write!(f, "AgentEvent::Started"),
            AgentEvent::Progress(s) => f.debug_tuple("AgentEvent::Progress").field(s).finish(),
            AgentEvent::Completed(r) => f.debug_tuple("AgentEvent::Completed").field(r).finish(),
            AgentEvent::UserInputRequired {
                question, options, ..
            } => f
                .debug_struct("AgentEvent::UserInputRequired")
                .field("question", question)
                .field("options", options)
                .finish(),
        }
    }
}

// Manual Clone impl for AgentEvent due to oneshot::Sender
impl Clone for AgentEvent {
    fn clone(&self) -> Self {
        match self {
            AgentEvent::Started => AgentEvent::Started,
            AgentEvent::Progress(s) => AgentEvent::Progress(s.clone()),
            AgentEvent::Completed(r) => AgentEvent::Completed(r.clone()),
            AgentEvent::UserInputRequired {
                question, options, ..
            } => {
                // Cannot clone oneshot::Sender, create a new event without the channel
                // This is acceptable since UserInputRequired is only sent, not cloned
                AgentEvent::Progress(format!(
                    "UserInputRequired: {} (options: {})",
                    question,
                    options.len()
                ))
            }
        }
    }
}

/// Context for agent execution with sub-session isolation.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Parent session identifier.
    pub session_id: SessionId,
    /// Unique sub-session identifier for this agent invocation.
    pub sub_session_id: SubSessionId,
    /// Task identifier (if applicable).
    pub task_id: Option<TaskId>,
    /// Agent type for this context.
    pub agent_type: AgentType,
    /// Maximum number of retries allowed.
    pub max_retries: usize,
    /// Whether to enable test-aware behavior.
    pub test_aware: bool,
    /// Additional metadata for the context.
    pub metadata: std::collections::HashMap<String, String>,
}

impl AgentContext {
    /// Creates a new agent context with a unique sub-session ID.
    pub fn new(
        session_id: SessionId,
        task_id: Option<TaskId>,
        agent_type: AgentType,
        max_retries: usize,
        test_aware: bool,
    ) -> Self {
        Self {
            session_id,
            sub_session_id: SubSessionId::new_v4(),
            task_id,
            agent_type,
            max_retries,
            test_aware,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Gets a metadata value by key.
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Sets a metadata value.
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
}

/// Common runner contract for all agents.
#[async_trait]
pub trait AgentRunner: Debug + Send + Sync {
    /// Returns the agent's unique identifier.
    fn id(&self) -> AgentId;

    /// Runs the agent with the given context.
    ///
    /// The `tx` channel is used to send progress events to the TUI/session layer.
    async fn run(
        &self,
        context: AgentContext,
        tx: tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> AgentResult;
}

/// Alias for agent message history.
pub type AgentMessageHistory = Vec<(String, String)>;

pub mod ask_agent;
pub mod code_agent;
pub mod plan_agent;
pub mod review_agent;
