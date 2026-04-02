// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

//! Plan Agent implementation with LLM integration, MCP tools, and Qdrant codebase search.
//!
//! This module implements the Plan Agent which:
//! - Runs clarification rounds with the user via LLM
//! - Uses context7 and sequential-thinking MCP tools for evidence gathering
//! - Searches the codebase via Qdrant for relevant existing code
//! - Generates structured plan files in the PRD-specified markdown format

use super::{AgentContext, AgentEvent, AgentId, AgentResult, AgentRunner};
use crate::llm::client::LlmClient;
use crate::llm::types::{ChatCompletionRequest, Message};
use async_trait::async_trait;
use index::embedder::Embedder;
use index::store::QdrantStore;
use mcp::registry::McpRegistry;
use serde::{Deserialize, Serialize};
use shared::config::LlmConfig;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use tracing::*;

/// Configuration for the Plan Agent.
#[derive(Debug, Clone)]
pub struct PlanAgentConfig {
    /// Maximum number of clarification rounds.
    pub max_clarification_rounds: usize,
    /// Path to the directory where plans should be saved.
    pub plans_dir: String,
    /// Whether to use MCP tools for evidence gathering.
    pub use_mcp_tools: bool,
    /// Confidence threshold for ambiguity detection (0.0-1.0).
    /// If LLM confidence is below this threshold, continue clarifying.
    pub ambiguity_threshold: f64,
    /// Number of top results to include from Qdrant search.
    pub qdrant_top_k: usize,
}

impl Default for PlanAgentConfig {
    fn default() -> Self {
        Self {
            max_clarification_rounds: 3,
            plans_dir: "plans".to_string(),
            use_mcp_tools: true,
            ambiguity_threshold: 0.8,
            qdrant_top_k: 5,
        }
    }
}

/// Clarification round result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClarificationResult {
    /// The question asked by the LLM.
    question: String,
    /// The user's answer.
    answer: Option<String>,
    /// LLM's confidence score (0.0-1.0).
    confidence: f64,
    /// Whether the LLM needs more clarification.
    needs_more_clarification: bool,
}

/// Context gathered from codebase search.
#[derive(Debug, Clone)]
struct CodebaseContext {
    /// Relevant file paths found in the codebase.
    pub relevant_files: Vec<String>,
    /// Summary of existing patterns and conventions.
    pub patterns_summary: String,
    /// Raw search results as formatted string.
    pub raw_results: String,
}

/// Plan Agent implementation.
pub struct PlanAgent {
    id: AgentId,
    config: PlanAgentConfig,
    llm_client: Option<Arc<LlmClient>>,
    mcp_registry: Option<Arc<McpRegistry>>,
    qdrant_store: Option<Arc<QdrantStore>>,
    embedder: Option<Arc<Embedder>>,
}

impl fmt::Debug for PlanAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlanAgent")
            .field("id", &self.id)
            .field("config", &self.config)
            .field("llm_client", &self.llm_client.as_ref().map(|_| "LlmClient"))
            .field(
                "mcp_registry",
                &self.mcp_registry.as_ref().map(|_| "McpRegistry"),
            )
            .field(
                "qdrant_store",
                &self.qdrant_store.as_ref().map(|_| "QdrantStore"),
            )
            .field("embedder", &self.embedder.as_ref().map(|_| "Embedder"))
            .finish()
    }
}

impl Clone for PlanAgent {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
            llm_client: self.llm_client.clone(),
            mcp_registry: self.mcp_registry.clone(),
            qdrant_store: self.qdrant_store.clone(),
            embedder: self.embedder.clone(),
        }
    }
}

impl PlanAgent {
    /// Creates a new Plan Agent instance with all dependencies.
    pub fn new(
        id: impl Into<AgentId>,
        config: Option<PlanAgentConfig>,
        llm_client: Option<Arc<LlmClient>>,
        mcp_registry: Option<Arc<McpRegistry>>,
        qdrant_store: Option<Arc<QdrantStore>>,
        embedder: Option<Arc<Embedder>>,
    ) -> Self {
        Self {
            id: id.into(),
            config: config.unwrap_or_default(),
            llm_client,
            mcp_registry,
            qdrant_store,
            embedder,
        }
    }

    /// Creates a new Plan Agent with just the LLM client (for simpler setups).
    pub fn with_llm(
        id: impl Into<AgentId>,
        config: Option<PlanAgentConfig>,
        llm_config: LlmConfig,
    ) -> Self {
        Self {
            id: id.into(),
            config: config.unwrap_or_default(),
            llm_client: Some(Arc::new(LlmClient::new(llm_config))),
            mcp_registry: None,
            qdrant_store: None,
            embedder: None,
        }
    }

    /// Builds the system prompt for plan generation.
    fn build_system_prompt(&self, codebase_context: Option<&CodebaseContext>) -> String {
        let mut prompt = r#"You are Telisq's Plan Agent, responsible for creating detailed implementation plans from user requirements.

## Your Role
1. Understand user requirements through clarification rounds
2. Break down complex requirements into atomic, testable tasks
3. Generate a structured plan in the required markdown format

## Plan Format Requirements
Each task in the plan MUST follow this exact format:
- [ ] Task title (sequential_number)
  Files: path/to/file1, path/to/file2
  Contract: Clear, testable contract description
  Depends on: task_number (if applicable)

## Rules
- Task IDs must be sequential integers starting from 1
- Each task should be atomic and independently testable
- File paths should be relative to the project root
- Contracts should be specific and verifiable
- Dependencies should reference other task IDs
- Avoid duplicate implementations - check existing codebase first

## Output Format
Generate ONLY the plan content in the specified format. Do not include explanations or commentary."#.to_string();

        if let Some(ctx) = codebase_context {
            prompt.push_str(&format!(
                r#"

## Existing Codebase Context
The following relevant files and patterns were found in the codebase:

### Relevant Files
{}

### Patterns and Conventions
{}

### Search Results Summary
{}

Use this context to avoid duplicating existing implementations and to follow established patterns."#,
                ctx.relevant_files.join("\n"),
                ctx.patterns_summary,
                ctx.raw_results
            ));
        }

        prompt
    }

    /// Builds the clarification prompt for a round.
    fn build_clarification_prompt(
        &self,
        task_description: &str,
        previous_rounds: &[ClarificationResult],
    ) -> Vec<Message> {
        let mut messages = vec![Message::system(
            r#"You are a requirements clarification assistant. Your goal is to understand the user's requirements thoroughly before creating an implementation plan.

Ask focused, specific questions that will help you understand:
1. The exact scope of work needed
2. File locations and naming conventions
3. Dependencies and constraints
4. Expected behavior and edge cases
5. Testing requirements

After each user answer, assess your confidence in understanding the requirements (0.0-1.0).
If confidence is below 0.8, ask another clarifying question.
If you have enough information, respond with "CLARIFICATION_COMPLETE" and list the key requirements you've understood."#,
        )];

        // Add task description
        messages.push(Message::user(&format!(
            "Here is the task/goal I need to plan: {}\n\nPlease start by asking your first clarifying question.",
            task_description
        )));

        // Add previous rounds
        for round in previous_rounds {
            messages.push(Message::assistant(&round.question));
            if let Some(answer) = &round.answer {
                messages.push(Message::user(answer));
            }
        }

        messages
    }

    /// Parses the LLM response to extract clarification question and confidence.
    fn parse_clarification_response(response: &str) -> ClarificationResult {
        // Check if clarification is complete
        if response.contains("CLARIFICATION_COMPLETE") {
            return ClarificationResult {
                question: String::new(),
                answer: None,
                confidence: 1.0,
                needs_more_clarification: false,
            };
        }

        // Extract confidence if present (look for patterns like "Confidence: 0.85")
        let confidence = response
            .lines()
            .find(|line| line.to_lowercase().contains("confidence"))
            .and_then(|line| {
                line.split(':')
                    .nth(1)
                    .and_then(|s| s.trim().parse::<f64>().ok())
            })
            .unwrap_or(0.7); // Default confidence if not specified

        // The question is typically the main content of the response
        let question = response
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.to_lowercase().contains("confidence"))
            .collect::<Vec<_>>()
            .join("\n");

        ClarificationResult {
            question,
            answer: None,
            confidence,
            needs_more_clarification: true,
        }
    }

    /// Runs clarification rounds with the user via LLM.
    async fn run_clarification_rounds(
        &self,
        context: &AgentContext,
        task_description: &str,
        tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> AgentResult {
        info!(
            session_id = %context.session_id,
            max_rounds = %self.config.max_clarification_rounds,
            "Starting clarification rounds"
        );

        let llm_client = match &self.llm_client {
            Some(client) => client,
            None => {
                error!("LLM client not configured");
                return AgentResult::Failure("LLM client not available".to_string());
            }
        };

        let mut rounds: Vec<ClarificationResult> = Vec::new();
        let mut current_round = 0;

        while current_round < self.config.max_clarification_rounds {
            current_round += 1;

            // Build prompt for this round
            let messages = self.build_clarification_prompt(task_description, &rounds);

            let request = ChatCompletionRequest {
                messages,
                tools: None,
                tool_choice: None,
                stream: false,
            };

            // Send to LLM
            let response = match llm_client.chat_completion(request).await {
                Ok(resp) => resp,
                Err(e) => {
                    error!(error = %e, "LLM clarification request failed");
                    tx.send(AgentEvent::Progress(format!(
                        "Clarification round {} failed: {}",
                        current_round, e
                    )))
                    .await
                    .ok();
                    continue;
                }
            };

            // Parse response
            let content = response
                .choices
                .first()
                .and_then(|c| c.message.content.clone())
                .unwrap_or_default();

            let parsed = Self::parse_clarification_response(&content);

            info!(
                round = %current_round,
                confidence = %parsed.confidence,
                needs_more = %parsed.needs_more_clarification,
                "Clarification round completed"
            );

            tx.send(AgentEvent::Progress(format!(
                "Clarification round {}/{}: {}",
                current_round,
                self.config.max_clarification_rounds,
                if parsed.needs_more_clarification {
                    &parsed.question
                } else {
                    "Requirements understood"
                }
            )))
            .await
            .ok();

            // Check if we need more clarification
            if !parsed.needs_more_clarification {
                info!("Clarification complete after {} rounds", current_round);
                break;
            }

            // Check confidence threshold
            if parsed.confidence >= self.config.ambiguity_threshold {
                info!(
                    confidence = %parsed.confidence,
                    threshold = %self.config.ambiguity_threshold,
                    "Confidence threshold met"
                );
                break;
            }

            // Store this round's result
            rounds.push(parsed);
        }

        AgentResult::Success(serde_json::json!({
            "clarification_completed": true,
            "rounds": current_round,
            "max_rounds": self.config.max_clarification_rounds,
            "clarification_history": rounds
        }))
    }

    /// Searches Qdrant for relevant codebase context.
    async fn search_codebase(&self, query: &str) -> Option<CodebaseContext> {
        let embedder = self.embedder.as_ref()?;
        let qdrant_store = self.qdrant_store.as_ref()?;

        info!(query, "Searching codebase for relevant context");

        // Generate embedding for the query
        let embedding = match embedder.embed(query).await {
            Ok(embedding) => embedding,
            Err(e) => {
                warn!(error = %e, "Failed to generate embedding for codebase search");
                return None;
            }
        };

        // Search Qdrant for similar content
        let results = match qdrant_store
            .search(embedding, self.config.qdrant_top_k)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                warn!(error = %e, "Qdrant search failed");
                return None;
            }
        };

        if results.is_empty() {
            info!("No relevant codebase context found");
            return None;
        }

        // Extract relevant files and patterns from results
        let mut relevant_files: Vec<String> = Vec::new();
        let mut patterns: Vec<String> = Vec::new();
        let mut raw_results: Vec<String> = Vec::new();

        for scored_point in &results {
            if let Some(file_path) = scored_point.payload.get("file_path") {
                if let Some(path_str) = file_path.as_str() {
                    if !relevant_files.contains(&path_str.to_string()) {
                        relevant_files.push(path_str.to_string());
                    }
                }
            }

            if let Some(content) = scored_point.payload.get("content") {
                if let Some(content_str) = content.as_str() {
                    raw_results.push(format!(
                        "File: {:?}\nScore: {:.3}\nContent: {}...\n",
                        scored_point.payload.get("file_path"),
                        scored_point.score,
                        &content_str[..content_str.len().min(200)]
                    ));
                }
            }

            if let Some(pattern) = scored_point.payload.get("pattern_type") {
                if let Some(pattern_str) = pattern.as_str() {
                    patterns.push(pattern_str.to_string());
                }
            }
        }

        let patterns_summary = if patterns.is_empty() {
            "No specific patterns identified".to_string()
        } else {
            patterns.join(", ")
        };

        info!(
            files_found = %relevant_files.len(),
            "Codebase search complete"
        );

        Some(CodebaseContext {
            relevant_files,
            patterns_summary,
            raw_results: raw_results.join("\n"),
        })
    }

    /// Uses MCP tools (context7, sequential-thinking) to gather additional context.
    async fn gather_mcp_context(&self, task_description: &str) -> String {
        if !self.config.use_mcp_tools {
            return String::new();
        }

        let registry = match &self.mcp_registry {
            Some(reg) => reg,
            None => {
                warn!("MCP registry not available, skipping tool usage");
                return String::new();
            }
        };

        let mut context_parts: Vec<String> = Vec::new();

        // Try to use context7 for documentation lookup
        let available_tools = registry.available_tools().await;
        info!(tools = ?available_tools, "Available MCP tools");

        // Attempt context7 documentation search
        if available_tools
            .iter()
            .any(|t| t.contains("context7") || t.contains("search"))
        {
            info!("Using context7 for documentation lookup");
            match registry
                .dispatch_tool_call(
                    "context7_search",
                    serde_json::json!({ "query": task_description }),
                )
                .await
            {
                Ok(result) => {
                    context_parts.push(format!("Context7 documentation results:\n{}", result));
                }
                Err(e) => {
                    warn!(error = %e, "context7 search failed, continuing without it");
                }
            }
        }

        // Attempt sequential-thinking for complex requirement breakdown
        if available_tools
            .iter()
            .any(|t| t.contains("sequential") || t.contains("thinking"))
        {
            info!("Using sequential-thinking for requirement breakdown");
            match registry
                .dispatch_tool_call(
                    "sequential_thinking",
                    serde_json::json!({
                        "thought": format!("Break down this requirement into implementation steps: {}", task_description),
                        "thought_number": 1,
                        "total_thoughts": 3
                    }),
                )
                .await
            {
                Ok(result) => {
                    context_parts.push(format!("Sequential thinking results:\n{}", result));
                }
                Err(e) => {
                    warn!(error = %e, "sequential-thinking failed, continuing without it");
                }
            }
        }

        context_parts.join("\n\n")
    }

    /// Generates a plan based on clarified requirements.
    async fn generate_plan(
        &self,
        context: &AgentContext,
        task_description: &str,
        tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> AgentResult {
        info!(
            session_id = %context.session_id,
            "Starting plan generation"
        );

        tx.send(AgentEvent::Progress(
            "Gathering codebase context...".to_string(),
        ))
        .await
        .ok();

        // Search codebase for relevant context
        let codebase_context = self.search_codebase(task_description).await;

        tx.send(AgentEvent::Progress(
            "Gathering MCP tool context...".to_string(),
        ))
        .await
        .ok();

        // Gather MCP tool context
        let mcp_context = self.gather_mcp_context(task_description).await;

        // Build system prompt with all context
        let system_prompt = self.build_system_prompt(codebase_context.as_ref());

        // Build user message with task description and MCP context
        let mut user_content = format!(
            "Generate an implementation plan for the following task:\n\n{}\n\n",
            task_description
        );

        if !mcp_context.is_empty() {
            user_content.push_str(&format!(
                "## Additional Context from MCP Tools\n{}\n\n",
                mcp_context
            ));
        }

        user_content.push_str("Please generate the plan in the required format.");

        let messages = vec![
            Message::system(&system_prompt),
            Message::user(&user_content),
        ];

        let request = ChatCompletionRequest {
            messages,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        // Get plan from LLM
        let llm_client = match &self.llm_client {
            Some(client) => client,
            None => {
                error!("LLM client not configured");
                return AgentResult::Failure("LLM client not available".to_string());
            }
        };

        let response = match llm_client.chat_completion(request).await {
            Ok(resp) => resp,
            Err(e) => {
                error!(error = %e, "LLM plan generation failed");
                return AgentResult::Failure(format!("Plan generation failed: {}", e));
            }
        };

        let plan_content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        info!(plan_length = %plan_content.len(), "Plan generation completed");

        tx.send(AgentEvent::Progress(
            "Plan generated successfully".to_string(),
        ))
        .await
        .ok();

        AgentResult::Success(serde_json::json!({
            "plan_generated": true,
            "plan_content": plan_content,
            "codebase_context_available": codebase_context.is_some(),
            "mcp_context_available": !mcp_context.is_empty()
        }))
    }

    /// Saves the generated plan to disk.
    async fn save_plan(
        &self,
        context: &AgentContext,
        plan_content: &str,
        tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> AgentResult {
        let plan_path = format!("{}/plan-{}.md", self.config.plans_dir, context.session_id);

        info!(path = %plan_path, "Saving plan to disk");

        // Ensure directory exists
        if let Some(parent) = Path::new(&plan_path).parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    error!(error = %e, "Failed to create plans directory");
                    return AgentResult::Failure(format!(
                        "Failed to create plans directory: {}",
                        e
                    ));
                }
            }
        }

        // Write plan to temp file then rename for atomicity
        let temp_path = format!("{}.tmp", plan_path);
        if let Err(e) = tokio::fs::write(&temp_path, plan_content).await {
            error!(error = %e, "Failed to write plan file");
            return AgentResult::Failure(format!("Failed to write plan file: {}", e));
        }

        if let Err(e) = tokio::fs::rename(&temp_path, &plan_path).await {
            error!(error = %e, "Failed to rename plan file");
            return AgentResult::Failure(format!("Failed to rename plan file: {}", e));
        }

        info!(path = %plan_path, "Plan saved successfully");

        tx.send(AgentEvent::Progress(format!("Plan saved to {}", plan_path)))
            .await
            .ok();

        AgentResult::Success(serde_json::json!({
            "plan_saved": true,
            "path": plan_path
        }))
    }
}

#[async_trait]
impl AgentRunner for PlanAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
    }

    async fn run(
        &self,
        context: AgentContext,
        tx: tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> AgentResult {
        tx.send(AgentEvent::Started).await.ok();
        tx.send(AgentEvent::Progress(
            "Starting plan generation process".to_string(),
        ))
        .await
        .ok();

        // Extract task description from context or use default
        let task_description = context
            .task_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "Implement the user's requested feature".to_string());

        // Run clarification rounds
        let clarification_result = self
            .run_clarification_rounds(&context, &task_description, &tx)
            .await;

        match clarification_result {
            AgentResult::Success(_) => {
                tx.send(AgentEvent::Progress(
                    "Clarification rounds completed".to_string(),
                ))
                .await
                .ok();
            }
            AgentResult::Failure(msg) => {
                tx.send(AgentEvent::Completed(AgentResult::Failure(msg.clone())))
                    .await
                    .ok();
                return AgentResult::Failure(format!("Clarification rounds failed: {}", msg));
            }
            _ => {
                tx.send(AgentEvent::Completed(AgentResult::Failure(
                    "Unexpected result from clarification rounds".to_string(),
                )))
                .await
                .ok();
                return AgentResult::Failure(
                    "Unexpected result from clarification rounds".to_string(),
                );
            }
        }

        // Generate plan
        let plan_result = self.generate_plan(&context, &task_description, &tx).await;

        match plan_result {
            AgentResult::Success(plan_data) => {
                let plan_content = plan_data
                    .get("plan_content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Plan content")
                    .to_string();

                tx.send(AgentEvent::Progress(
                    "Plan generation completed".to_string(),
                ))
                .await
                .ok();

                // Save plan to disk
                let save_result = self.save_plan(&context, &plan_content, &tx).await;
                match save_result {
                    AgentResult::Success(_) => {
                        tx.send(AgentEvent::Progress("Plan saved to disk".to_string()))
                            .await
                            .ok();

                        AgentResult::Success(plan_data)
                    }
                    AgentResult::Failure(msg) => AgentResult::Failure(msg),
                    _ => AgentResult::Failure("Unexpected result from plan saving".to_string()),
                }
            }
            AgentResult::Failure(msg) => AgentResult::Failure(msg),
            _ => AgentResult::Failure("Unexpected result from plan generation".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clarification_result_parsing_complete() {
        let response = "CLARIFICATION_COMPLETE\n\nKey requirements understood:\n- User authentication needed\n- OAuth2 flow required";
        let result = PlanAgent::parse_clarification_response(response);
        assert!(!result.needs_more_clarification);
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_clarification_result_parsing_with_confidence() {
        let response = "What authentication method should be used?\n\nConfidence: 0.65";
        let result = PlanAgent::parse_clarification_response(response);
        assert!(result.needs_more_clarification);
        assert!((result.confidence - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_clarification_result_parsing_default_confidence() {
        let response = "What is the expected behavior?";
        let result = PlanAgent::parse_clarification_response(response);
        assert!(result.needs_more_clarification);
        assert!((result.confidence - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_system_prompt_without_codebase_context() {
        let agent = PlanAgent::new(
            "test-agent",
            Some(PlanAgentConfig::default()),
            None,
            None,
            None,
            None,
        );
        let prompt = agent.build_system_prompt(None);
        assert!(prompt.contains("Plan Agent"));
        assert!(prompt.contains("Plan Format Requirements"));
        assert!(!prompt.contains("Existing Codebase Context"));
    }

    #[test]
    fn test_system_prompt_with_codebase_context() {
        let agent = PlanAgent::new(
            "test-agent",
            Some(PlanAgentConfig::default()),
            None,
            None,
            None,
            None,
        );
        let ctx = CodebaseContext {
            relevant_files: vec!["src/main.rs".to_string()],
            patterns_summary: "Rust async patterns".to_string(),
            raw_results: "Search results...".to_string(),
        };
        let prompt = agent.build_system_prompt(Some(&ctx));
        assert!(prompt.contains("Existing Codebase Context"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("Rust async patterns"));
    }

    #[test]
    fn test_plan_agent_debug_format() {
        let agent = PlanAgent::new(
            "test-agent",
            Some(PlanAgentConfig::default()),
            None,
            None,
            None,
            None,
        );
        let debug_str = format!("{:?}", agent);
        assert!(debug_str.contains("PlanAgent"));
        assert!(debug_str.contains("test-agent"));
    }
}
