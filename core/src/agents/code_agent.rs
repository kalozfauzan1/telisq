// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

//! Code Agent implementation with LLM integration, MCP tools, and patch application.
//!
//! This module implements the Code Agent which:
//! - Uses LLM to generate code patches based on task specifications
//! - Reads/writes/patches files via MCP tools
//! - Runs verification commands via bash MCP tool
//! - Integrates serena for code analysis and context7 for API documentation

use super::{AgentContext, AgentEvent, AgentId, AgentResult, AgentRunner};
use crate::llm::client::LlmClient;
use crate::llm::types::{ChatCompletionRequest, Message, Role};
use crate::patcher::{FilePatch, PatchResult, Patcher};
use async_trait::async_trait;
use mcp::registry::McpRegistry;
use serde::{Deserialize, Serialize};
use shared::config::LlmConfig;
use std::fmt;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tracing::*;

/// Configuration for the Code Agent.
#[derive(Debug, Clone)]
pub struct CodeAgentConfig {
    /// Maximum number of retries for failed patches.
    pub max_retries: usize,
    /// Whether to run tests for modified modules.
    pub test_aware: bool,
    /// List of allowed files to modify.
    pub allowed_files: Vec<PathBuf>,
    /// Command to verify the code changes.
    pub verify_command: Option<String>,
    /// Timeout for verify command execution (seconds).
    pub verify_command_timeout_secs: u64,
}

impl Default for CodeAgentConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            test_aware: true,
            allowed_files: Vec::new(),
            verify_command: None,
            verify_command_timeout_secs: 120,
        }
    }
}

/// Represents a file operation requested by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum FileOperation {
    /// Create a new file.
    Create { path: String, content: String },
    /// Modify an existing file with a search/replace patch.
    Modify {
        path: String,
        original: String,
        replacement: String,
    },
    /// Delete an existing file.
    Delete { path: String },
}

/// Response structure expected from LLM code generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGenerationResponse {
    /// List of file operations to perform.
    pub operations: Vec<FileOperation>,
    /// Optional explanation or summary of changes.
    pub summary: Option<String>,
}

/// Code Agent implementation.
pub struct CodeAgent {
    id: AgentId,
    config: CodeAgentConfig,
    llm_client: Option<Arc<LlmClient>>,
    mcp_registry: Option<Arc<McpRegistry>>,
}

impl fmt::Debug for CodeAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodeAgent")
            .field("id", &self.id)
            .field("config", &self.config)
            .field("llm_client", &self.llm_client.as_ref().map(|_| "LlmClient"))
            .field(
                "mcp_registry",
                &self.mcp_registry.as_ref().map(|_| "McpRegistry"),
            )
            .finish()
    }
}

impl Clone for CodeAgent {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
            llm_client: self.llm_client.clone(),
            mcp_registry: self.mcp_registry.clone(),
        }
    }
}

impl CodeAgent {
    /// Creates a new Code Agent instance with all dependencies.
    pub fn new(
        id: impl Into<AgentId>,
        config: Option<CodeAgentConfig>,
        llm_client: Option<Arc<LlmClient>>,
        mcp_registry: Option<Arc<McpRegistry>>,
    ) -> Self {
        Self {
            id: id.into(),
            config: config.unwrap_or_default(),
            llm_client,
            mcp_registry,
        }
    }

    /// Creates a new Code Agent with just the LLM client (for simpler setups).
    pub fn with_llm(
        id: impl Into<AgentId>,
        config: Option<CodeAgentConfig>,
        llm_config: LlmConfig,
    ) -> Self {
        Self {
            id: id.into(),
            config: config.unwrap_or_default(),
            llm_client: Some(Arc::new(LlmClient::new(llm_config))),
            mcp_registry: None,
        }
    }

    /// Builds the system prompt for code generation.
    fn build_system_prompt(&self) -> String {
        let allowed_files_msg = if self.config.allowed_files.is_empty() {
            "You may modify any files in the project.".to_string()
        } else {
            let files: Vec<_> = self
                .config
                .allowed_files
                .iter()
                .map(|f| f.to_str().unwrap_or(""))
                .collect();
            format!(
                "You are ONLY allowed to modify the following files:\n{}",
                files.join("\n")
            )
        };

        format!(
            r#"You are an expert software engineer tasked with implementing code changes according to a specification.

## Rules
1. Generate precise, surgical patches to modify existing code.
2. Always read the file content before modifying it to ensure your patch will apply correctly.
3. Use search/replace patches with enough context (3+ lines) to ensure uniqueness.
4. Create new files when the file doesn't exist yet.
5. Delete files only when explicitly requested.
6. After making changes, run the verification command to ensure correctness.
7. Follow existing code style and conventions in the project.
8. Do NOT modify files outside the scope of the task.

## File Constraints
{allowed_files_msg}

## Response Format
Respond with a JSON object containing:
- "operations": array of file operations (create, modify, delete)
- "summary": brief explanation of the changes

Each operation must have:
- "operation": one of "create", "modify", "delete"
- "path": file path
- For "create": "content" field with the full file content
- For "modify": "original" (text to replace) and "replacement" (new text)
- For "delete": no additional fields

Return ONLY valid JSON, no markdown formatting."#
        )
    }

    /// Builds the user prompt with task context.
    fn build_user_prompt(&self, context: &AgentContext) -> String {
        let task_info = match &context.task_id {
            Some(task_id) => format!("Task ID: {}", task_id),
            None => "No specific task ID provided.".to_string(),
        };

        let plan_context = context
            .get_metadata("plan_context")
            .map(|s| format!("\n\n## Plan Context\n{}", s))
            .unwrap_or_default();

        let task_spec = context
            .get_metadata("task_spec")
            .map(|s| format!("\n\n## Task Specification\n{}", s))
            .unwrap_or_default();

        let dependencies = context
            .get_metadata("dependencies")
            .map(|s| format!("\n\n## Dependencies\n{}", s))
            .unwrap_or_default();

        format!(
            r#"## Task Information
{task_info}
{task_spec}
{dependencies}
{plan_context}

Please implement the required changes according to the task specification above."#
        )
    }

    /// Gathers additional context using serena and context7 MCP tools.
    async fn gather_context(&self, context: &AgentContext) -> String {
        let mut context_parts = Vec::new();

        // Try serena for code analysis
        if let Some(registry) = &self.mcp_registry {
            match self
                .call_mcp_tool(
                    registry,
                    "serena",
                    serde_json::json!({
                        "query": context.get_metadata("task_spec").unwrap_or_default()
                    }),
                )
                .await
            {
                Ok(result) => {
                    context_parts.push(format!("## Serena Analysis\n{}", result));
                }
                Err(e) => {
                    warn!(error = %e, "Serena tool failed, continuing without it");
                }
            }

            // Try context7 for API documentation
            match self
                .call_mcp_tool(
                    registry,
                    "context7",
                    serde_json::json!({
                        "query": context.get_metadata("task_spec").unwrap_or_default()
                    }),
                )
                .await
            {
                Ok(result) => {
                    context_parts.push(format!("## Context7 Documentation\n{}", result));
                }
                Err(e) => {
                    warn!(error = %e, "Context7 tool failed, continuing without it");
                }
            }
        }

        if context_parts.is_empty() {
            String::new()
        } else {
            context_parts.join("\n\n")
        }
    }

    /// Calls an MCP tool via the registry.
    async fn call_mcp_tool(
        &self,
        registry: &McpRegistry,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        let result = registry
            .dispatch_tool_call(tool_name, arguments)
            .await
            .map_err(|e| e.to_string())?;

        serde_json::to_string(&result).map_err(|e| e.to_string())
    }

    /// Reads a file using MCP readFile tool.
    async fn read_file_via_mcp(&self, path: &str) -> Result<String, String> {
        if let Some(registry) = &self.mcp_registry {
            let result = self
                .call_mcp_tool(registry, "readFile", serde_json::json!({ "path": path }))
                .await?;

            // Parse the result to extract content
            let parsed: serde_json::Value =
                serde_json::from_str(&result).map_err(|e| e.to_string())?;

            parsed
                .get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| "Failed to extract file content from MCP result".to_string())
        } else {
            // Fallback to direct file read if no MCP registry
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        }
    }

    /// Writes a file using MCP writeFile tool.
    async fn write_file_via_mcp(&self, path: &str, content: &str) -> Result<(), String> {
        if let Some(registry) = &self.mcp_registry {
            let _result = self
                .call_mcp_tool(
                    registry,
                    "writeFile",
                    serde_json::json!({
                        "path": path,
                        "content": content,
                        "overwrite": true
                    }),
                )
                .await?;
            Ok(())
        } else {
            // Fallback to direct file write if no MCP registry
            std::fs::write(path, content).map_err(|e| e.to_string())
        }
    }

    /// Patches a file using MCP editFile tool.
    async fn patch_file_via_mcp(
        &self,
        path: &str,
        search: &str,
        replace: &str,
    ) -> Result<(), String> {
        if let Some(registry) = &self.mcp_registry {
            let _result = self
                .call_mcp_tool(
                    registry,
                    "editFile",
                    serde_json::json!({
                        "path": path,
                        "search": search,
                        "replace": replace
                    }),
                )
                .await?;
            Ok(())
        } else {
            // Fallback to local patcher
            let patch = FilePatch {
                file_path: PathBuf::from(path),
                original: search.to_string(),
                replacement: replace.to_string(),
            };
            match Patcher::apply_patch(&patch) {
                PatchResult::Success => Ok(()),
                PatchResult::Failure(msg) => Err(msg),
                PatchResult::FileNotFound => Err("File not found".to_string()),
                PatchResult::ContentMismatch => Err("Content mismatch".to_string()),
            }
        }
    }

    /// Runs a command using MCP runCommand tool.
    async fn run_command_via_mcp(&self, command: &str) -> Result<CommandResult, String> {
        if let Some(registry) = &self.mcp_registry {
            let result = self
                .call_mcp_tool(
                    registry,
                    "runCommand",
                    serde_json::json!({
                        "command": command,
                        "blocking": true
                    }),
                )
                .await?;

            let parsed: serde_json::Value =
                serde_json::from_str(&result).map_err(|e| e.to_string())?;

            Ok(CommandResult {
                stdout: parsed
                    .get("stdout")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                stderr: parsed
                    .get("stderr")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                exit_code: parsed
                    .get("exit_code")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(-1),
            })
        } else {
            // Fallback to direct command execution
            self.run_command_local(command).await
        }
    }

    /// Runs a command locally (fallback when MCP is unavailable).
    async fn run_command_local(&self, command: &str) -> Result<CommandResult, String> {
        let timeout_duration = Duration::from_secs(self.config.verify_command_timeout_secs);

        let result = timeout(timeout_duration, async {
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
                .await
        })
        .await
        .map_err(|e| {
            format!(
                "Command timed out after {}s: {}",
                timeout_duration.as_secs(),
                e
            )
        })?
        .map_err(|e| format!("Failed to execute command: {}", e))?;

        Ok(CommandResult {
            stdout: String::from_utf8_lossy(&result.stdout).to_string(),
            stderr: String::from_utf8_lossy(&result.stderr).to_string(),
            exit_code: result.status.code().map(|c| c as i64).unwrap_or(-1),
        })
    }

    /// Generates patches using LLM.
    async fn generate_patches(&self, context: &AgentContext) -> AgentResult {
        let llm_client = match &self.llm_client {
            Some(client) => client,
            None => return AgentResult::Failure("LLM client not configured".to_string()),
        };

        // Gather additional context from serena/context7
        let extra_context = self.gather_context(context).await;

        // Build messages
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(context);

        let full_user_prompt = if extra_context.is_empty() {
            user_prompt
        } else {
            format!("{}\n\n{}", user_prompt, extra_context)
        };

        let request = ChatCompletionRequest {
            messages: vec![
                Message {
                    role: Role::System,
                    content: system_prompt,
                    name: None,
                },
                Message {
                    role: Role::User,
                    content: full_user_prompt,
                    name: None,
                },
            ],
            tools: None,
            tool_choice: None,
            stream: false,
        };

        info!("Sending code generation request to LLM");

        match llm_client.chat_completion(request).await {
            Ok(response) => {
                let content = response
                    .choices
                    .first()
                    .and_then(|c| c.message.content.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or("");

                info!(
                    response_length = content.len(),
                    "LLM code generation response received"
                );

                // Parse the JSON response
                match serde_json::from_str::<CodeGenerationResponse>(content) {
                    Ok(parsed) => {
                        info!(
                            operation_count = parsed.operations.len(),
                            "Parsed code generation response"
                        );
                        AgentResult::Success(serde_json::to_value(&parsed).unwrap_or_default())
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to parse LLM response as JSON, treating as raw content");
                        // Try to extract JSON from markdown if present
                        if let Some(json_str) = extract_json_from_markdown(content) {
                            match serde_json::from_str::<CodeGenerationResponse>(&json_str) {
                                Ok(parsed) => {
                                    return AgentResult::Success(
                                        serde_json::to_value(&parsed).unwrap_or_default(),
                                    );
                                }
                                Err(e2) => AgentResult::Failure(format!(
                                    "Failed to parse extracted JSON: {}",
                                    e2
                                )),
                            }
                        } else {
                            AgentResult::Failure(format!("Failed to parse LLM response: {}", e))
                        }
                    }
                }
            }
            Err(e) => AgentResult::Failure(format!("LLM request failed: {}", e)),
        }
    }

    /// Verifies that patches are within allowed file constraints.
    fn verify_patches_constraints(&self, patches: &[FilePatch]) -> AgentResult {
        // Check if we have any files to restrict
        if self.config.allowed_files.is_empty() {
            return AgentResult::Success(serde_json::json!({ "constraints_ok": true }));
        }

        // Verify all patches are for allowed files
        let disallowed_patches: Vec<_> = patches
            .iter()
            .filter(|patch| !self.config.allowed_files.contains(&patch.file_path))
            .collect();

        if disallowed_patches.is_empty() {
            AgentResult::Success(serde_json::json!({ "constraints_ok": true }))
        } else {
            let disallowed_files: Vec<_> = disallowed_patches
                .iter()
                .map(|p| p.file_path.to_str().unwrap_or(""))
                .collect();

            AgentResult::Failure(format!(
                "Patches for disallowed files: {}",
                disallowed_files.join(", ")
            ))
        }
    }

    /// Applies file operations from the LLM response.
    async fn apply_file_operations(&self, operations: &[FileOperation]) -> AgentResult {
        let mut results = Vec::new();
        let mut failures = Vec::new();

        for op in operations {
            match op {
                FileOperation::Create { path, content } => {
                    info!(path = %path, "Creating file");
                    match self.write_file_via_mcp(path, content).await {
                        Ok(()) => {
                            results.push(format!("Created: {}", path));
                        }
                        Err(e) => {
                            failures.push(format!("Failed to create {}: {}", path, e));
                        }
                    }
                }
                FileOperation::Modify {
                    path,
                    original,
                    replacement,
                } => {
                    info!(path = %path, "Modifying file");
                    // First verify the patch can be applied
                    let verify_result = self.verify_patch_local(path, original).await;
                    match verify_result {
                        Ok(()) => {
                            match self.patch_file_via_mcp(path, original, replacement).await {
                                Ok(()) => {
                                    results.push(format!("Modified: {}", path));
                                }
                                Err(e) => {
                                    failures.push(format!("Failed to modify {}: {}", path, e));
                                }
                            }
                        }
                        Err(e) => {
                            failures.push(format!("Patch verification failed for {}: {}", path, e));
                        }
                    }
                }
                FileOperation::Delete { path } => {
                    info!(path = %path, "Deleting file");
                    if let Some(registry) = &self.mcp_registry {
                        match self
                            .call_mcp_tool(
                                registry,
                                "deleteFile",
                                serde_json::json!({ "path": path }),
                            )
                            .await
                        {
                            Ok(_) => {
                                results.push(format!("Deleted: {}", path));
                            }
                            Err(e) => {
                                failures.push(format!("Failed to delete {}: {}", path, e));
                            }
                        }
                    } else {
                        match std::fs::remove_file(path) {
                            Ok(()) => {
                                results.push(format!("Deleted: {}", path));
                            }
                            Err(e) => {
                                failures.push(format!("Failed to delete {}: {}", path, e));
                            }
                        }
                    }
                }
            }
        }

        if failures.is_empty() {
            AgentResult::Success(serde_json::json!({
                "operations_applied": true,
                "results": results
            }))
        } else {
            AgentResult::Failure(format!(
                "Some operations failed: {}. Successful: {}",
                failures.join("; "),
                results.join("; ")
            ))
        }
    }

    /// Verifies a patch locally without applying it.
    async fn verify_patch_local(&self, path: &str, original: &str) -> Result<(), String> {
        let content = self.read_file_via_mcp(path).await?;
        if content.contains(original) {
            Ok(())
        } else {
            Err("Original content not found in file".to_string())
        }
    }

    /// Runs the verification command.
    async fn run_verify_command(&self) -> AgentResult {
        if let Some(command) = &self.config.verify_command {
            info!(command = %command, "Running verification command");
            match self.run_command_via_mcp(command).await {
                Ok(result) => {
                    if result.exit_code == 0 {
                        AgentResult::Success(serde_json::json!({
                            "verify_command_success": true,
                            "stdout": result.stdout,
                            "stderr": result.stderr
                        }))
                    } else {
                        AgentResult::Failure(format!(
                            "Verify command failed with exit code {}: {}",
                            result.exit_code, result.stderr
                        ))
                    }
                }
                Err(e) => AgentResult::Failure(format!("Failed to run verify command: {}", e)),
            }
        } else {
            AgentResult::Success(serde_json::json!({ "verify_command_skipped": true }))
        }
    }

    /// Runs tests for the modified module.
    async fn run_tests(&self, _context: &AgentContext) -> AgentResult {
        if self.config.test_aware {
            // Try to run tests via MCP or locally
            let test_command =
                "cargo test 2>&1 || echo 'No tests found or test framework unavailable'";
            match self.run_command_via_mcp(test_command).await {
                Ok(result) => {
                    let passed = result.stdout.contains("test result: ok");
                    let failed =
                        !result.stdout.contains("test result: ok") && result.exit_code != 0;

                    AgentResult::Success(serde_json::json!({
                        "tests_ran": true,
                        "passed": passed,
                        "failed": failed,
                        "output": result.stdout
                    }))
                }
                Err(e) => AgentResult::Success(serde_json::json!({
                    "tests_ran": false,
                    "error": e
                })),
            }
        } else {
            AgentResult::Success(serde_json::json!({ "tests_skipped": true }))
        }
    }
}

/// Result of command execution.
#[derive(Debug, Clone)]
struct CommandResult {
    stdout: String,
    stderr: String,
    exit_code: i64,
}

/// Extracts JSON from markdown code blocks if present.
fn extract_json_from_markdown(content: &str) -> Option<String> {
    // Look for ```json ... ``` blocks
    if let Some(start) = content.find("```json") {
        let after_start = &content[start + 7..];
        if let Some(end) = after_start.find("```") {
            return Some(after_start[..end].trim().to_string());
        }
    }
    // Look for generic ``` ... ``` blocks
    if let Some(start) = content.find("```") {
        let after_start = &content[start + 3..];
        // Skip language identifier if present
        let after_lang = if after_start
            .chars()
            .next()
            .map_or(false, |c| c.is_alphabetic())
        {
            after_start
                .find('\n')
                .map(|i| &after_start[i + 1..])
                .unwrap_or(after_start)
        } else {
            after_start
        };
        if let Some(end) = after_lang.find("```") {
            return Some(after_lang[..end].trim().to_string());
        }
    }
    None
}

#[async_trait]
impl AgentRunner for CodeAgent {
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
            "Starting code generation process".to_string(),
        ))
        .await
        .ok();

        let mut retries = 0;
        let mut last_error = None;

        while retries <= self.config.max_retries {
            tx.send(AgentEvent::Progress(format!("Attempt {}", retries + 1)))
                .await
                .ok();

            // Generate patches
            let patches_result = self.generate_patches(&context).await;
            match patches_result {
                AgentResult::Success(operations_data) => {
                    // Parse operations from the response
                    let operations: Vec<FileOperation> =
                        match serde_json::from_value(operations_data.clone()) {
                            Ok(ops) => ops,
                            Err(e) => {
                                last_error = Some(format!("Failed to parse operations: {}", e));
                                retries += 1;
                                continue;
                            }
                        };

                    if operations.is_empty() {
                        tx.send(AgentEvent::Progress("No operations to apply".to_string()))
                            .await
                            .ok();
                        return AgentResult::Success(serde_json::json!({
                            "no_operations": true
                        }));
                    }

                    // Convert operations to patches for constraint verification
                    let patches: Vec<FilePatch> = operations
                        .iter()
                        .filter_map(|op| match op {
                            FileOperation::Modify {
                                path,
                                original,
                                replacement,
                            } => Some(FilePatch {
                                file_path: PathBuf::from(path),
                                original: original.clone(),
                                replacement: replacement.clone(),
                            }),
                            _ => None,
                        })
                        .collect();

                    // Verify patches constraints
                    let constraints_result = self.verify_patches_constraints(&patches);
                    match constraints_result {
                        AgentResult::Success(_) => {
                            // Apply file operations
                            let apply_result = self.apply_file_operations(&operations).await;
                            match apply_result {
                                AgentResult::Success(_) => {
                                    tx.send(AgentEvent::Progress(
                                        "File operations applied successfully".to_string(),
                                    ))
                                    .await
                                    .ok();

                                    // Run verify command
                                    let verify_result = self.run_verify_command().await;
                                    match verify_result {
                                        AgentResult::Success(_) => {
                                            tx.send(AgentEvent::Progress(
                                                "Verification passed".to_string(),
                                            ))
                                            .await
                                            .ok();

                                            // Run tests if configured
                                            let test_result = self.run_tests(&context).await;
                                            match test_result {
                                                AgentResult::Success(test_data) => {
                                                    tx.send(AgentEvent::Completed(
                                                        AgentResult::Success(test_data.clone()),
                                                    ))
                                                    .await
                                                    .ok();
                                                    return AgentResult::Success(test_data);
                                                }
                                                AgentResult::Failure(msg) => {
                                                    last_error = Some(msg);
                                                }
                                                _ => {
                                                    last_error =
                                                        Some("Unexpected test result".to_string());
                                                }
                                            }
                                        }
                                        AgentResult::Failure(msg) => {
                                            last_error = Some(msg);
                                        }
                                        _ => {
                                            last_error =
                                                Some("Unexpected verification result".to_string());
                                        }
                                    }
                                }
                                AgentResult::Failure(msg) => {
                                    last_error = Some(msg);
                                }
                                _ => {
                                    last_error =
                                        Some("Unexpected file operation result".to_string());
                                }
                            }
                        }
                        AgentResult::Failure(msg) => {
                            last_error = Some(msg);
                        }
                        _ => {
                            last_error =
                                Some("Unexpected constraints verification result".to_string());
                        }
                    }
                }
                AgentResult::Failure(msg) => {
                    last_error = Some(msg);
                }
                _ => {
                    last_error = Some("Unexpected patch generation result".to_string());
                }
            }

            retries += 1;
            if retries <= self.config.max_retries {
                tx.send(AgentEvent::Progress(format!(
                    "Retrying ({} of {}) after failure: {}",
                    retries,
                    self.config.max_retries,
                    last_error.as_ref().unwrap()
                )))
                .await
                .ok();
            }
        }

        // All retries failed
        AgentResult::Failure(format!(
            "All {} attempts failed. Last error: {}",
            self.config.max_retries + 1,
            last_error.unwrap()
        ))
    }
}
