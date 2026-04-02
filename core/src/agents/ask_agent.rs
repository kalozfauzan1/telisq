// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

//! Ask Agent implementation with LLM integration for question formulation.
//!
//! This module implements the Ask Agent which:
//! - Uses LLM to formulate concise, focused questions
//! - Displays questions and options to the user via TUI
//! - Accepts user input via keyboard (predefined options or free text)
//! - Handles input timeout with configurable limit
//! - Returns user answer to orchestrator

use super::{AgentContext, AgentEvent, AgentId, AgentResult, AgentRunner, AgentUserOption};
use crate::llm::client::LlmClient;
use crate::llm::types::{ChatCompletionRequest, Message, Role};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shared::config::LlmConfig;
use std::fmt;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};
use tracing::*;

/// User input options for the Ask Agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOption {
    /// Unique identifier for the option.
    pub id: String,
    /// Display text for the option.
    pub text: String,
    /// Whether the option allows free text input.
    pub allow_free_text: bool,
}

impl UserOption {
    /// Creates a new user option.
    pub fn new(id: impl Into<String>, text: impl Into<String>, allow_free_text: bool) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            allow_free_text,
        }
    }
}

/// Configuration for the Ask Agent.
#[derive(Debug, Clone)]
pub struct AskAgentConfig {
    /// Timeout in seconds for user input.
    pub input_timeout: u64,
    /// Whether to allow free text input when no options are selected.
    pub allow_free_text: bool,
    /// Whether to use LLM for question formulation.
    pub use_llm: bool,
}

impl Default for AskAgentConfig {
    fn default() -> Self {
        Self {
            input_timeout: 300, // 5 minutes
            allow_free_text: true,
            use_llm: true,
        }
    }
}

/// Question formulation request for the LLM.
#[derive(Debug, Clone)]
struct QuestionFormulationRequest {
    /// The base question to formulate.
    pub question: String,
    /// Available options for the user.
    pub options: Vec<UserOption>,
    /// Context about the current task/situation.
    pub context: String,
}

/// LLM-formulated question result.
#[derive(Debug, Clone)]
struct FormulatedQuestion {
    /// The formulated question text.
    pub question: String,
    /// Formatted options for display.
    pub formatted_options: String,
}

/// Ask Agent implementation.
pub struct AskAgent {
    id: AgentId,
    config: AskAgentConfig,
    llm_client: Option<Arc<LlmClient>>,
}

impl fmt::Debug for AskAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AskAgent")
            .field("id", &self.id)
            .field("config", &self.config)
            .field("llm_client", &self.llm_client.as_ref().map(|_| "LlmClient"))
            .finish()
    }
}

impl Clone for AskAgent {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
            llm_client: self.llm_client.clone(),
        }
    }
}

impl AskAgent {
    /// Creates a new Ask Agent instance with all dependencies.
    pub fn new(
        id: impl Into<AgentId>,
        config: Option<AskAgentConfig>,
        llm_client: Option<Arc<LlmClient>>,
    ) -> Self {
        Self {
            id: id.into(),
            config: config.unwrap_or_default(),
            llm_client,
        }
    }

    /// Creates a new Ask Agent with just the LLM client (for simpler setups).
    pub fn with_llm(
        id: impl Into<AgentId>,
        config: Option<AskAgentConfig>,
        llm_config: LlmConfig,
    ) -> Self {
        Self {
            id: id.into(),
            config: config.unwrap_or_default(),
            llm_client: Some(Arc::new(LlmClient::new(llm_config))),
        }
    }

    /// Builds the system prompt for question formulation.
    fn build_question_system_prompt(&self) -> String {
        r#"You are an assistant that helps formulate clear, concise questions for users.
Your task is to take a base question and context, and formulate it into a well-structured question that:
1. Is clear and unambiguous
2. Includes relevant context to help the user make an informed decision
3. Is concise and focused
4. Presents options in a structured format when applicable

Format your response as JSON with the following structure:
{
    "question": "The formulated question text",
    "options_summary": "Brief summary of available options"
}

Do not include any additional text outside of the JSON structure."#
            .to_string()
    }

    /// Builds the user prompt for question formulation.
    fn build_question_user_prompt(&self, request: &QuestionFormulationRequest) -> String {
        let options_text = request
            .options
            .iter()
            .map(|o| format!("- {}: {}", o.id, o.text))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Base question: {}

Available options:
{}

Context:
{}

Please formulate a clear, concise question that incorporates the context and presents the options effectively."#,
            request.question, options_text, request.context
        )
    }

    /// Uses LLM to formulate a question with context.
    async fn formulate_question_with_llm(
        &self,
        request: &QuestionFormulationRequest,
    ) -> Result<FormulatedQuestion, String> {
        let Some(ref client) = self.llm_client else {
            warn!("LLM client not available, using default question formulation");
            return self.formulate_question_default(&request);
        };

        let system_prompt = self.build_question_system_prompt();
        let user_prompt = self.build_question_user_prompt(&request);

        let messages = vec![
            Message {
                role: Role::System,
                content: system_prompt,
                name: None,
            },
            Message {
                role: Role::User,
                content: user_prompt,
                name: None,
            },
        ];

        let chat_request = ChatCompletionRequest {
            messages,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        match client.chat_completion(chat_request).await {
            Ok(response) => {
                let content = response
                    .choices
                    .first()
                    .and_then(|c| c.message.content.as_ref())
                    .unwrap_or(&request.question);

                // Try to parse JSON response
                match self.parse_formulated_question(content) {
                    Ok(formulated) => {
                        info!(
                            question = %formulated.question,
                            "LLM-formulated question"
                        );
                        Ok(formulated)
                    }
                    Err(e) => {
                        warn!(
                            error = %e,
                            "Failed to parse LLM response, using default"
                        );
                        self.formulate_question_default(&request)
                    }
                }
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "LLM question formulation failed, using default"
                );
                self.formulate_question_default(&request)
            }
        }
    }

    /// Parses the LLM's formulated question from JSON.
    fn parse_formulated_question(&self, content: &str) -> Result<FormulatedQuestion, String> {
        // Try to extract JSON from markdown if present
        let json_str = Self::extract_json_from_markdown(content);

        #[derive(Deserialize)]
        struct LlmQuestionResponse {
            question: String,
            #[serde(default)]
            options_summary: String,
        }

        let parsed: LlmQuestionResponse = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse LLM response as JSON: {}", e))?;

        Ok(FormulatedQuestion {
            question: parsed.question,
            formatted_options: parsed.options_summary,
        })
    }

    /// Extracts JSON from markdown content if present.
    fn extract_json_from_markdown(content: &str) -> &str {
        // Look for JSON code block
        if let Some(start) = content.find("```json") {
            let after_start = &content[start + 7..];
            if let Some(end) = after_start.find("```") {
                return after_start[..end].trim();
            }
        }

        // Look for any code block
        if let Some(start) = content.find("```") {
            let after_start = &content[start + 3..];
            // Skip language identifier if present
            let content_start = after_start.find('\n').map(|i| i + 1).unwrap_or(0);
            let block_content = &after_start[content_start..];
            if let Some(end) = block_content.find("```") {
                return block_content[..end].trim();
            }
        }

        // Try to find JSON object directly
        content.trim()
    }

    /// Default question formulation without LLM.
    fn formulate_question_default(
        &self,
        request: &QuestionFormulationRequest,
    ) -> Result<FormulatedQuestion, String> {
        let options_text = request
            .options
            .iter()
            .map(|o| format!("- [{}] {}", o.id, o.text))
            .collect::<Vec<_>>()
            .join("\n");

        let question = if request.context.is_empty() {
            request.question.clone()
        } else {
            format!("{}\n\nContext: {}", request.question, request.context)
        };

        let formatted_options = format!("Available options:\n{}", options_text);

        Ok(FormulatedQuestion {
            question,
            formatted_options,
        })
    }

    /// Formats the question and options for TUI display.
    fn format_for_display(&self, question: &str, options: &[UserOption]) -> String {
        let options_text = options
            .iter()
            .map(|o| format!("[{}] {}", o.id, o.text))
            .collect::<Vec<_>>()
            .join("\n");

        if options.is_empty() {
            question.to_string()
        } else {
            format!(
                "{}\n\n{}\n\nType an option ID or enter free text:",
                question, options_text
            )
        }
    }

    /// Handles user input and validates it against available options.
    async fn handle_user_input(&self, response: &str, options: &[UserOption]) -> AgentResult {
        // Check if response matches any predefined option
        let matching_option = options
            .iter()
            .find(|option| option.id == response || option.text == response);

        if let Some(option) = matching_option {
            AgentResult::Success(serde_json::json!({
                "response": option.id,
                "free_text": false,
                "option_id": option.id,
                "option_text": option.text
            }))
        } else if self.config.allow_free_text || options.is_empty() {
            // If no matching option and free text is allowed, treat as free text
            AgentResult::Success(serde_json::json!({
                "response": response.to_string(),
                "free_text": true,
                "text": response.to_string()
            }))
        } else {
            AgentResult::Failure(
                "Invalid response. Please select from the available options.".to_string(),
            )
        }
    }

    /// Builds context from agent context for question formulation.
    fn build_context_from_agent_context(&self, ctx: &AgentContext) -> String {
        let mut context_parts = Vec::new();

        if let Some(task_id) = &ctx.task_id {
            context_parts.push(format!("Task: {}", task_id));
        }

        context_parts.push(format!("Session: {}", ctx.session_id));

        if ctx.max_retries > 0 {
            context_parts.push(format!("Max retries: {}", ctx.max_retries));
        }

        if !ctx.metadata.is_empty() {
            let metadata_str = ctx
                .metadata
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            context_parts.push(format!("Metadata: {}", metadata_str));
        }

        context_parts.join("\n")
    }
}

#[async_trait]
impl AgentRunner for AskAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
    }

    async fn run(
        &self,
        context: AgentContext,
        tx: tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> AgentResult {
        info!(
            agent_id = %self.id,
            task_id = ?context.task_id,
            "Ask Agent starting"
        );

        tx.send(AgentEvent::Started).await.ok();
        tx.send(AgentEvent::Progress("Waiting for user input".to_string()))
            .await
            .ok();

        // Build the question formulation request
        let context_str = self.build_context_from_agent_context(&context);

        // Default question and options (can be customized via metadata)
        let base_question = context
            .get_metadata("question")
            .unwrap_or("How would you like to proceed?")
            .to_string();

        // Parse options from metadata if available
        let options = if let Some(options_json) = context.get_metadata("options") {
            serde_json::from_str::<Vec<UserOption>>(options_json).unwrap_or_else(|e| {
                warn!(error = %e, "Failed to parse options from metadata, using defaults");
                vec![
                    UserOption::new("retry", "Retry the task", false),
                    UserOption::new("skip", "Skip the task", false),
                    UserOption::new("stop", "Stop the execution", false),
                ]
            })
        } else {
            vec![
                UserOption::new("retry", "Retry the task", false),
                UserOption::new("skip", "Skip the task", false),
                UserOption::new("stop", "Stop the execution", false),
            ]
        };

        // Formulate question (with LLM if available)
        let question_request = QuestionFormulationRequest {
            question: base_question.clone(),
            options: options.clone(),
            context: context_str,
        };

        let formulated = if self.config.use_llm {
            match self.formulate_question_with_llm(&question_request).await {
                Ok(formulated_q) => formulated_q,
                Err(_) => {
                    warn!("LLM formulation failed, using default");
                    match self.formulate_question_default(&question_request) {
                        Ok(q) => q,
                        Err(_) => FormulatedQuestion {
                            question: base_question.clone(),
                            formatted_options: String::new(),
                        },
                    }
                }
            }
        } else {
            match self.formulate_question_default(&question_request) {
                Ok(q) => q,
                Err(_) => FormulatedQuestion {
                    question: base_question.clone(),
                    formatted_options: String::new(),
                },
            }
        };

        // Format for display
        let display_text = self.format_for_display(&formulated.question, &options);

        info!(
            question = %formulated.question,
            options_count = %options.len(),
            "Displaying question to user"
        );

        // Send the question to TUI via event
        tx.send(AgentEvent::Progress(display_text.clone()))
            .await
            .ok();

        // Wait for user input with timeout
        let timeout_secs = self.config.input_timeout;
        info!(
            timeout_secs = %timeout_secs,
            "Waiting for user input with timeout"
        );

        // Create a oneshot channel for receiving the user's answer
        let (answer_tx, answer_rx) = oneshot::channel();

        // Convert options to AgentUserOption for the event
        let agent_options: Vec<AgentUserOption> = options
            .iter()
            .map(|o| AgentUserOption {
                id: o.id.clone(),
                text: o.text.clone(),
                allow_free_text: o.allow_free_text,
            })
            .collect();

        // Send UserInputRequired event to TUI
        tx.send(AgentEvent::UserInputRequired {
            question: display_text,
            options: agent_options,
            answer_tx,
        })
        .await
        .ok();

        // Wait for user input with timeout
        match timeout(Duration::from_secs(timeout_secs), answer_rx).await {
            Ok(Ok(user_response)) => {
                info!(
                    response = %user_response,
                    "Received user input"
                );

                let handle_result = self.handle_user_input(&user_response, &options).await;
                match handle_result {
                    AgentResult::Success(processed_response) => {
                        tx.send(AgentEvent::Completed(AgentResult::Success(
                            processed_response.clone(),
                        )))
                        .await
                        .ok();
                        AgentResult::Success(processed_response)
                    }
                    AgentResult::Failure(msg) => {
                        tx.send(AgentEvent::Completed(AgentResult::Failure(msg.clone())))
                            .await
                            .ok();
                        AgentResult::Failure(msg)
                    }
                    _ => {
                        let error_msg = "Unexpected result from input handling".to_string();
                        tx.send(AgentEvent::Completed(AgentResult::Failure(
                            error_msg.clone(),
                        )))
                        .await
                        .ok();
                        AgentResult::Failure(error_msg)
                    }
                }
            }
            Ok(Err(_)) => {
                let error_msg = "Failed to receive user input".to_string();
                tx.send(AgentEvent::Completed(AgentResult::Failure(
                    error_msg.clone(),
                )))
                .await
                .ok();
                AgentResult::Failure(error_msg)
            }
            Err(_) => {
                warn!(
                    timeout_secs = %timeout_secs,
                    "User input timed out"
                );
                tx.send(AgentEvent::Completed(AgentResult::Failure(format!(
                    "User input timed out after {} seconds",
                    timeout_secs
                ))))
                .await
                .ok();
                AgentResult::Failure(format!(
                    "User input timed out after {} seconds",
                    timeout_secs
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_option_creation() {
        let option = UserOption::new("retry", "Retry the task", false);
        assert_eq!(option.id, "retry");
        assert_eq!(option.text, "Retry the task");
        assert!(!option.allow_free_text);
    }

    #[test]
    fn test_ask_agent_config_defaults() {
        let config = AskAgentConfig::default();
        assert_eq!(config.input_timeout, 300);
        assert!(config.allow_free_text);
        assert!(config.use_llm);
    }

    #[test]
    fn test_format_for_display_with_options() {
        let agent = AskAgent::new("test", None, None);
        let options = vec![
            UserOption::new("retry", "Retry", false),
            UserOption::new("skip", "Skip", false),
        ];
        let display = agent.format_for_display("What now?", &options);
        assert!(display.contains("What now?"));
        assert!(display.contains("[retry] Retry"));
        assert!(display.contains("[skip] Skip"));
    }

    #[test]
    fn test_build_context_from_agent_context() {
        let agent = AskAgent::new("test", None, None);
        let mut ctx = AgentContext::new(
            shared::types::SessionId::new_v4(),
            Some("task-1".to_string()),
            shared::brief::AgentType::Ask,
            3,
            true,
        );
        ctx.metadata.insert("key".to_string(), "value".to_string());

        let context_str = agent.build_context_from_agent_context(&ctx);
        assert!(context_str.contains("task-1"));
        assert!(context_str.contains("key: value"));
    }

    #[test]
    fn test_handle_user_input_matching_option() {
        let agent = AskAgent::new("test", None, None);
        let options = vec![
            UserOption::new("retry", "Retry", false),
            UserOption::new("skip", "Skip", false),
        ];

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(agent.handle_user_input("retry", &options));

        match result {
            AgentResult::Success(val) => {
                assert_eq!(val["response"], "retry");
                assert_eq!(val["free_text"], false);
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn test_handle_user_input_free_text() {
        let config = AskAgentConfig {
            allow_free_text: true,
            ..Default::default()
        };
        let agent = AskAgent::new("test", Some(config), None);
        let options = vec![
            UserOption::new("retry", "Retry", false),
            UserOption::new("skip", "Skip", false),
        ];

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(agent.handle_user_input("custom response", &options));

        match result {
            AgentResult::Success(val) => {
                assert_eq!(val["free_text"], true);
                assert_eq!(val["text"], "custom response");
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn test_handle_user_input_invalid_no_free_text() {
        let config = AskAgentConfig {
            allow_free_text: false,
            ..Default::default()
        };
        let agent = AskAgent::new("test", Some(config), None);
        let options = vec![UserOption::new("retry", "Retry", false)];

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(agent.handle_user_input("invalid", &options));

        match result {
            AgentResult::Failure(msg) => {
                assert!(msg.contains("Invalid response"));
            }
            _ => panic!("Expected failure"),
        }
    }

    #[test]
    fn test_extract_json_from_markdown() {
        // JSON code block
        let json_block = r#"```json
{"question": "Test?", "options_summary": "Options"}
```"#;
        let extracted = AskAgent::extract_json_from_markdown(json_block);
        assert!(extracted.contains("\"question\""));

        // Plain JSON
        let plain_json = r#"{"question": "Test?", "options_summary": "Options"}"#;
        let extracted = AskAgent::extract_json_from_markdown(plain_json);
        assert!(extracted.contains("\"question\""));
    }
}
