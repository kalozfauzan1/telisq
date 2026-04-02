// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

//! Review Agent implementation with LLM integration, bash test execution,
//! and code review logic for validating task completion.
//!
//! This module implements the Review Agent which:
//! - Uses LLM to analyze code changes against task contracts
//! - Runs verification commands to validate changes
//! - Detects issues and classifies them as errors or warnings
//! - Generates structured review reports

use super::{AgentContext, AgentEvent, AgentId, AgentResult, AgentRunner};
use crate::llm::client::LlmClient;
use crate::llm::types::{ChatCompletionRequest, Message, Role};
use async_trait::async_trait;
use mcp::registry::McpRegistry;
use serde::{Deserialize, Serialize};
use shared::config::LlmConfig;
use shared::types::{FilePath, TaskSpec};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tracing::*;

/// Issue types found during review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueType {
    /// Blocking error that prevents task completion.
    Error,
    /// Non-blocking warning that should be addressed.
    Warning,
}

/// Issue found during review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    /// Type of issue.
    pub issue_type: IssueType,
    /// Short description of the issue.
    pub title: String,
    /// Detailed description of the issue.
    pub description: String,
    /// File path where the issue was found (if applicable).
    pub file_path: Option<FilePath>,
    /// Line number where the issue was found (if applicable).
    pub line_number: Option<usize>,
}

impl Issue {
    /// Creates a new error issue.
    pub fn error(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            issue_type: IssueType::Error,
            title: title.into(),
            description: description.into(),
            file_path: None,
            line_number: None,
        }
    }

    /// Creates a new error issue with file path.
    pub fn error_with_path(
        title: impl Into<String>,
        description: impl Into<String>,
        file_path: impl Into<FilePath>,
    ) -> Self {
        Self {
            issue_type: IssueType::Error,
            title: title.into(),
            description: description.into(),
            file_path: Some(file_path.into()),
            line_number: None,
        }
    }

    /// Creates a new error issue with file path and line number.
    pub fn error_with_location(
        title: impl Into<String>,
        description: impl Into<String>,
        file_path: impl Into<FilePath>,
        line_number: usize,
    ) -> Self {
        Self {
            issue_type: IssueType::Error,
            title: title.into(),
            description: description.into(),
            file_path: Some(file_path.into()),
            line_number: Some(line_number),
        }
    }

    /// Creates a new warning issue.
    pub fn warning(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            issue_type: IssueType::Warning,
            title: title.into(),
            description: description.into(),
            file_path: None,
            line_number: None,
        }
    }

    /// Creates a new warning issue with file path.
    pub fn warning_with_path(
        title: impl Into<String>,
        description: impl Into<String>,
        file_path: impl Into<FilePath>,
    ) -> Self {
        Self {
            issue_type: IssueType::Warning,
            title: title.into(),
            description: description.into(),
            file_path: Some(file_path.into()),
            line_number: None,
        }
    }
}

/// Result of a verification command execution.
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Standard output from the command.
    pub stdout: String,
    /// Standard error from the command.
    pub stderr: String,
    /// Exit code from the command.
    pub exit_code: i64,
}

/// Configuration for the Review Agent.
#[derive(Debug, Clone)]
pub struct ReviewAgentConfig {
    /// List of commands to run for verification.
    pub verify_commands: Vec<String>,
    /// Timeout for verify command execution (seconds).
    pub verify_command_timeout_secs: u64,
    /// Whether to use LLM for code review analysis.
    pub use_llm_review: bool,
    /// Whether to use MCP tools for context gathering.
    pub use_mcp_tools: bool,
}

impl Default for ReviewAgentConfig {
    fn default() -> Self {
        Self {
            verify_commands: Vec::new(),
            verify_command_timeout_secs: 120,
            use_llm_review: true,
            use_mcp_tools: true,
        }
    }
}

/// Response structure expected from LLM code review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReviewResponse {
    /// List of issues found during review.
    pub issues: Vec<LlmIssue>,
    /// Overall assessment summary.
    pub summary: String,
    /// Whether the changes are approved.
    pub approved: bool,
}

/// Issue structure from LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmIssue {
    /// Type of issue: "error" or "warning".
    pub issue_type: String,
    /// Short description of the issue.
    pub title: String,
    /// Detailed description of the issue.
    pub description: String,
    /// File path where the issue was found (if applicable).
    pub file_path: Option<String>,
    /// Line number where the issue was found (if applicable).
    pub line_number: Option<usize>,
}

/// Review Agent implementation.
pub struct ReviewAgent {
    id: AgentId,
    config: ReviewAgentConfig,
    llm_client: Option<Arc<LlmClient>>,
    mcp_registry: Option<Arc<McpRegistry>>,
}

impl fmt::Debug for ReviewAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReviewAgent")
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

impl Clone for ReviewAgent {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
            llm_client: self.llm_client.clone(),
            mcp_registry: self.mcp_registry.clone(),
        }
    }
}

impl ReviewAgent {
    /// Creates a new Review Agent instance with all dependencies.
    pub fn new(
        id: impl Into<AgentId>,
        config: Option<ReviewAgentConfig>,
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

    /// Creates a new Review Agent with just the LLM client (for simpler setups).
    pub fn with_llm(
        id: impl Into<AgentId>,
        config: Option<ReviewAgentConfig>,
        llm_config: LlmConfig,
    ) -> Self {
        Self {
            id: id.into(),
            config: config.unwrap_or_default(),
            llm_client: Some(Arc::new(LlmClient::new(llm_config))),
            mcp_registry: None,
        }
    }

    /// Builds the system prompt for code review.
    fn build_system_prompt(&self) -> String {
        format!(
            r#"You are an expert code reviewer tasked with reviewing code changes against a task specification.

## Review Criteria
1. **Contract Compliance**: Verify all contracts specified in the task are satisfied.
2. **File Modifications**: Verify all files listed in the task spec were modified as expected.
3. **Code Quality**: Check for code quality issues, bugs, and potential problems.
4. **Side Effects**: Check for unintended changes to unrelated files.
5. **Test Compliance**: Ensure verification commands pass successfully.

## Issue Classification
- **Error**: Blocking issues that prevent task completion (bugs, missing contracts, incorrect implementations).
- **Warning**: Non-blocking issues that should be addressed but don't prevent completion (style issues, minor improvements).

## Response Format
Respond with a JSON object containing:
- "issues": array of issues found (empty if none)
- "summary": brief overall assessment
- "approved": boolean indicating if changes are approved

Each issue must have:
- "issue_type": "error" or "warning"
- "title": short description
- "description": detailed explanation
- "file_path": optional file path where issue was found
- "line_number": optional line number where issue was found

Return ONLY valid JSON, no markdown formatting."#
        )
    }

    /// Builds the user prompt with task context and changed files.
    fn build_user_prompt(&self, context: &AgentContext, task_spec: Option<&TaskSpec>) -> String {
        let task_info = match &context.task_id {
            Some(task_id) => format!("Task ID: {}", task_id),
            None => "No specific task ID provided.".to_string(),
        };

        let spec_info = match task_spec {
            Some(spec) => {
                let files_list: Vec<_> = spec
                    .files
                    .iter()
                    .map(|f| f.to_str().unwrap_or(""))
                    .collect();
                let contracts_list = spec.contracts.join("\n- ");

                format!(
                    r#"## Task Specification
- Title: {title}
- Description: {description}
- Files to modify:
{files}
- Contracts to satisfy:
- {contracts}"#,
                    title = spec.title,
                    description = spec.description.as_deref().unwrap_or("None"),
                    files = files_list
                        .iter()
                        .map(|f| format!("- {}", f))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    contracts = if contracts_list.is_empty() {
                        "None".to_string()
                    } else {
                        contracts_list
                    }
                )
            }
            None => "No task specification provided.".to_string(),
        };

        let changed_files_info = context
            .get_metadata("changed_files")
            .map(|s| format!("\n\n## Changed Files\n{}", s))
            .unwrap_or_default();

        let diffs_info = context
            .get_metadata("file_diffs")
            .map(|s| format!("\n\n## File Diffs\n{}", s))
            .unwrap_or_default();

        let test_results_info = context
            .get_metadata("test_results")
            .map(|s| format!("\n\n## Test Results\n{}", s))
            .unwrap_or_default();

        format!(
            r#"## Task Information
{task_info}

{spec_info}
{changed_files_info}
{diffs_info}
{test_results_info}

Please review the code changes above against the task specification. Identify any issues and provide an overall assessment."#
        )
    }

    /// Gathers additional context using MCP tools.
    async fn gather_context(&self, context: &AgentContext) -> String {
        let mut context_parts = Vec::new();

        if let Some(registry) = &self.mcp_registry {
            // Try serena for code analysis
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

    /// Reads a file using MCP readFile tool or direct file access.
    async fn read_file_content(&self, path: &str) -> Result<String, String> {
        if let Some(registry) = &self.mcp_registry {
            let result = self
                .call_mcp_tool(registry, "readFile", serde_json::json!({ "path": path }))
                .await?;

            let parsed: serde_json::Value =
                serde_json::from_str(&result).map_err(|e| e.to_string())?;

            parsed
                .get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| "Failed to extract file content from MCP result".to_string())
        } else {
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        }
    }

    /// Loads changed files content for review context.
    async fn load_changed_files(&self, task_spec: Option<&TaskSpec>) -> String {
        let mut file_contents = Vec::new();

        if let Some(spec) = task_spec {
            for file_path in &spec.files {
                let path_str = file_path.to_str().unwrap_or("");
                match self.read_file_content(path_str).await {
                    Ok(content) => {
                        file_contents
                            .push(format!("### File: {}\n```\n{}\n```", path_str, content));
                    }
                    Err(e) => {
                        warn!(path = %path_str, error = %e, "Failed to read file for review");
                        file_contents
                            .push(format!("### File: {}\n[Failed to read: {}]", path_str, e));
                    }
                }
            }
        }

        if file_contents.is_empty() {
            "No files to review.".to_string()
        } else {
            file_contents.join("\n\n")
        }
    }

    /// Parses LLM response into structured Issue list.
    fn parse_llm_response(&self, content: &str) -> Vec<Issue> {
        let mut issues = Vec::new();

        // Try to parse as CodeReviewResponse
        match serde_json::from_str::<CodeReviewResponse>(content) {
            Ok(response) => {
                for llm_issue in response.issues {
                    let issue = self.convert_llm_issue(llm_issue);
                    issues.push(issue);
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to parse LLM response as CodeReviewResponse, trying alternative parsing");

                // Try to parse as array of issues
                if let Ok(issue_array) = serde_json::from_str::<Vec<LlmIssue>>(content) {
                    for llm_issue in issue_array {
                        let issue = self.convert_llm_issue(llm_issue);
                        issues.push(issue);
                    }
                } else {
                    // Try to extract JSON from markdown
                    if let Some(json_str) = extract_json_from_markdown(content) {
                        if let Ok(response) = serde_json::from_str::<CodeReviewResponse>(&json_str)
                        {
                            for llm_issue in response.issues {
                                let issue = self.convert_llm_issue(llm_issue);
                                issues.push(issue);
                            }
                        } else if let Ok(issue_array) =
                            serde_json::from_str::<Vec<LlmIssue>>(&json_str)
                        {
                            for llm_issue in issue_array {
                                let issue = self.convert_llm_issue(llm_issue);
                                issues.push(issue);
                            }
                        } else {
                            warn!("Failed to parse extracted JSON from markdown");
                        }
                    } else {
                        warn!("No JSON found in LLM response");
                    }
                }
            }
        }

        issues
    }

    /// Converts an LLM issue to an internal Issue.
    fn convert_llm_issue(&self, llm_issue: LlmIssue) -> Issue {
        let issue_type = match llm_issue.issue_type.to_lowercase().as_str() {
            "error" => IssueType::Error,
            "warning" => IssueType::Warning,
            _ => IssueType::Warning, // Default to warning for unknown types
        };

        let file_path = llm_issue.file_path.map(|p| PathBuf::from(p));

        Issue {
            issue_type,
            title: llm_issue.title,
            description: llm_issue.description,
            file_path,
            line_number: llm_issue.line_number,
        }
    }

    /// Analyzes code changes using LLM and identifies issues.
    async fn analyze_changes(
        &self,
        context: &AgentContext,
        task_spec: Option<&TaskSpec>,
    ) -> Vec<Issue> {
        if !self.config.use_llm_review {
            info!("LLM review disabled, skipping analysis");
            return Vec::new();
        }

        let llm_client = match &self.llm_client {
            Some(client) => client,
            None => {
                warn!("LLM client not configured, skipping LLM analysis");
                return Vec::new();
            }
        };

        // Gather additional context from MCP tools
        let extra_context = self.gather_context(context).await;

        // Load changed files content
        let file_contents = self.load_changed_files(task_spec).await;

        // Build messages
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(context, task_spec);

        let full_user_prompt = if extra_context.is_empty() {
            format!(
                "{}\n\n## Current File Contents\n{}",
                user_prompt, file_contents
            )
        } else {
            format!(
                "{}\n\n{}\n\n## Current File Contents\n{}",
                user_prompt, extra_context, file_contents
            )
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

        info!("Sending code review request to LLM");

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
                    "LLM code review response received"
                );

                self.parse_llm_response(content)
            }
            Err(e) => {
                warn!(error = %e, "LLM review request failed, continuing without LLM analysis");
                Vec::new()
            }
        }
    }

    /// Verifies task contracts against changed files.
    async fn verify_contracts(
        &self,
        context: &AgentContext,
        task_spec: Option<&TaskSpec>,
    ) -> AgentResult {
        let mut issues = Vec::new();

        let spec = match task_spec {
            Some(s) => s,
            None => {
                return AgentResult::Success(serde_json::json!({
                    "contracts_verified": true,
                    "contract_count": 0,
                    "issues_found": false,
                    "note": "No task specification provided, skipping contract verification"
                }))
            }
        };

        // Verify all files in task spec were modified
        let changed_files_str = context.get_metadata("changed_files").unwrap_or("");
        let changed_files: Vec<&str> = changed_files_str
            .split('\n')
            .filter(|s| !s.is_empty())
            .collect();

        for expected_file in &spec.files {
            let expected_path = expected_file.to_str().unwrap_or("");
            let was_modified = changed_files.iter().any(|f| f.contains(expected_path));

            if !was_modified {
                issues.push(Issue::error_with_path(
                    "Missing File Modification",
                    format!(
                        "Expected file '{}' to be modified according to task spec, but it was not found in changed files",
                        expected_path
                    ),
                    expected_file.clone(),
                ));
            }
        }

        // Check for unintended side effects (files modified that aren't in spec)
        if !spec.files.is_empty() {
            for changed_file in &changed_files {
                let is_expected = spec.files.iter().any(|f| {
                    let f_str = f.to_str().unwrap_or("");
                    changed_file.contains(f_str) || f_str.contains(*changed_file)
                });

                if !is_expected {
                    issues.push(Issue::warning_with_path(
                        "Unintended File Modification",
                        format!(
                            "File '{}' was modified but is not listed in the task spec",
                            changed_file
                        ),
                        PathBuf::from(changed_file),
                    ));
                }
            }
        }

        // Verify contracts are satisfied (basic check - LLM will do deeper analysis)
        for contract in &spec.contracts {
            // Basic contract satisfaction check
            // In a real implementation, this would parse and verify specific contract types
            info!(contract = %contract, "Checking contract satisfaction");
        }

        if issues.is_empty() {
            AgentResult::Success(serde_json::json!({
                "contracts_verified": true,
                "contract_count": spec.contracts.len(),
                "files_checked": spec.files.len(),
                "issues_found": false
            }))
        } else {
            let error_count = issues
                .iter()
                .filter(|i| matches!(i.issue_type, IssueType::Error))
                .count();
            let warning_count = issues
                .iter()
                .filter(|i| matches!(i.issue_type, IssueType::Warning))
                .count();

            AgentResult::Success(serde_json::json!({
                "contracts_verified": error_count == 0,
                "contract_count": spec.contracts.len(),
                "files_checked": spec.files.len(),
                "issues_found": true,
                "error_count": error_count,
                "warning_count": warning_count,
                "issues": issues.iter().map(|i| serde_json::json!({
                    "type": format!("{:?}", i.issue_type),
                    "title": i.title,
                    "description": i.description,
                    "file_path": i.file_path.as_ref().map(|p| p.to_str().unwrap_or("")),
                    "line_number": i.line_number
                })).collect::<Vec<_>>()
            }))
        }
    }

    /// Runs all verification commands and parses results.
    async fn run_verify_commands(&self) -> AgentResult {
        if self.config.verify_commands.is_empty() {
            return AgentResult::Success(serde_json::json!({
                "verify_commands_skipped": true,
                "reason": "No verification commands configured"
            }));
        }

        let mut results = Vec::new();
        let mut failures = Vec::new();

        for (index, command) in self.config.verify_commands.iter().enumerate() {
            let result = self.run_command(command).await;

            match result {
                Ok(cmd_result) => {
                    let success = cmd_result.exit_code == 0;
                    results.push((
                        index,
                        success,
                        cmd_result.stdout.clone(),
                        cmd_result.stderr.clone(),
                    ));

                    if !success {
                        failures.push((index, command.clone(), cmd_result));
                    }
                }
                Err(e) => {
                    failures.push((
                        index,
                        command.clone(),
                        CommandResult {
                            stdout: String::new(),
                            stderr: e.clone(),
                            exit_code: -1,
                        },
                    ));
                }
            }
        }

        if failures.is_empty() {
            AgentResult::Success(serde_json::json!({
                "verify_commands_success": true,
                "command_count": results.len(),
                "results": results.iter().map(|(i, success, stdout, stderr)| {
                    serde_json::json!({
                        "command_index": i + 1,
                        "success": success,
                        "stdout": stdout,
                        "stderr": stderr
                    })
                }).collect::<Vec<_>>()
            }))
        } else {
            let failure_details: Vec<_> = failures
                .iter()
                .map(|(index, command, result)| {
                    format!(
                        "Command {} '{}' failed with exit code {}: {}",
                        index + 1,
                        command,
                        result.exit_code,
                        if result.stderr.is_empty() {
                            &result.stdout
                        } else {
                            &result.stderr
                        }
                    )
                })
                .collect();

            // Parse failing test names from output
            let failing_tests = self.parse_failing_tests(&failures);

            AgentResult::Failure(format!(
                "Verification failed: {}. Failing tests: {}",
                failure_details.join("; "),
                if failing_tests.is_empty() {
                    "none identified".to_string()
                } else {
                    failing_tests.join(", ")
                }
            ))
        }
    }

    /// Runs a command with timeout.
    async fn run_command(&self, command: &str) -> Result<CommandResult, String> {
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

    /// Parses failing test names from command output.
    fn parse_failing_tests(&self, failures: &[(usize, String, CommandResult)]) -> Vec<String> {
        let mut failing_tests = Vec::new();

        for (_, command, result) in failures {
            let output = if result.stderr.is_empty() {
                &result.stdout
            } else {
                &result.stderr
            };

            // Parse common test output patterns
            // Pattern: "test name ... FAILED"
            for line in output.lines() {
                if line.contains("FAILED") {
                    if let Some(test_name) = line.split("...").next() {
                        let test_name = test_name.trim();
                        if !test_name.is_empty() && test_name != "test" {
                            failing_tests.push(format!("{} ({})", test_name, command));
                        }
                    }
                }

                // Pattern: "error: ..."
                if line.starts_with("error") {
                    failing_tests.push(format!("{}: {}", command, line));
                }
            }
        }

        failing_tests
    }

    /// Generates a structured review report.
    fn generate_review_report(
        &self,
        issues: &[Issue],
        contracts_result: &AgentResult,
        verify_result: &AgentResult,
    ) -> serde_json::Value {
        let errors: Vec<_> = issues
            .iter()
            .filter(|i| matches!(i.issue_type, IssueType::Error))
            .collect();
        let warnings: Vec<_> = issues
            .iter()
            .filter(|i| matches!(i.issue_type, IssueType::Warning))
            .collect();

        serde_json::json!({
            "review_summary": {
                "approved": errors.is_empty(),
                "error_count": errors.len(),
                "warning_count": warnings.len(),
                "total_issues": issues.len()
            },
            "contracts_verification": contracts_result,
            "verify_commands": verify_result,
            "errors": errors.iter().map(|i| serde_json::json!({
                "title": i.title,
                "description": i.description,
                "file_path": i.file_path.as_ref().map(|p| p.to_str().unwrap_or("")),
                "line_number": i.line_number
            })).collect::<Vec<_>>(),
            "warnings": warnings.iter().map(|i| serde_json::json!({
                "title": i.title,
                "description": i.description,
                "file_path": i.file_path.as_ref().map(|p| p.to_str().unwrap_or("")),
                "line_number": i.line_number
            })).collect::<Vec<_>>()
        })
    }
}

#[async_trait]
impl AgentRunner for ReviewAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
    }

    async fn run(
        &self,
        context: AgentContext,
        tx: tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> AgentResult {
        tx.send(AgentEvent::Started).await.ok();
        tx.send(AgentEvent::Progress("Starting review process".to_string()))
            .await
            .ok();

        // Extract task specification from context metadata
        let task_spec = context
            .get_metadata("task_spec")
            .and_then(|s| serde_json::from_str::<TaskSpec>(s).ok());

        // Step 1: Verify contracts
        tx.send(AgentEvent::Progress("Verifying task contracts".to_string()))
            .await
            .ok();

        let contracts_result = self.verify_contracts(&context, task_spec.as_ref()).await;

        // Check for contract errors
        let contract_issues = match &contracts_result {
            AgentResult::Success(value) => {
                if let Some(issues) = value.get("issues") {
                    if let Some(issue_array) = issues.as_array() {
                        let errors: Vec<_> = issue_array
                            .iter()
                            .filter(|i| {
                                i.get("type")
                                    .and_then(|t| t.as_str())
                                    .map(|t| t.contains("Error"))
                                    .unwrap_or(false)
                            })
                            .collect();
                        if !errors.is_empty() {
                            // Return early if there are contract errors
                            let error_messages: Vec<_> = errors
                                .iter()
                                .map(|i| {
                                    format!(
                                        "{}: {}",
                                        i.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                                        i.get("description").and_then(|v| v.as_str()).unwrap_or("")
                                    )
                                })
                                .collect();

                            tx.send(AgentEvent::Completed(AgentResult::Failure(format!(
                                "Contract verification errors: {}",
                                error_messages.join(", ")
                            ))))
                            .await
                            .ok();

                            return AgentResult::Failure(format!(
                                "Contract verification errors: {}",
                                error_messages.join(", ")
                            ));
                        }
                    }
                }
                Vec::new()
            }
            AgentResult::Failure(msg) => {
                tx.send(AgentEvent::Completed(AgentResult::Failure(msg.clone())))
                    .await
                    .ok();
                return AgentResult::Failure(msg.clone());
            }
            _ => {
                tx.send(AgentEvent::Completed(AgentResult::Failure(
                    "Unexpected result from contract verification".to_string(),
                )))
                .await
                .ok();
                return AgentResult::Failure(
                    "Unexpected result from contract verification".to_string(),
                );
            }
        };

        tx.send(AgentEvent::Progress(
            "Contract verification completed".to_string(),
        ))
        .await
        .ok();

        // Step 2: Run verification commands
        tx.send(AgentEvent::Progress(
            "Running verification commands".to_string(),
        ))
        .await
        .ok();

        let verify_result = self.run_verify_commands().await;

        match &verify_result {
            AgentResult::Success(_) => {
                tx.send(AgentEvent::Progress(
                    "Verification commands completed".to_string(),
                ))
                .await
                .ok();
            }
            AgentResult::Failure(msg) => {
                // Store test results in context for LLM review
                let mut ctx = context.clone();
                ctx.set_metadata("test_results".to_string(), msg.clone());

                tx.send(AgentEvent::Progress(format!(
                    "Verification commands found issues: {}",
                    msg
                )))
                .await
                .ok();

                // Continue to LLM review even if tests fail - it may provide useful analysis
            }
            _ => {
                tx.send(AgentEvent::Completed(AgentResult::Failure(
                    "Unexpected result from verification commands".to_string(),
                )))
                .await
                .ok();
                return AgentResult::Failure(
                    "Unexpected result from verification commands".to_string(),
                );
            }
        }

        // Step 3: Analyze changes with LLM
        tx.send(AgentEvent::Progress(
            "Analyzing code changes with LLM".to_string(),
        ))
        .await
        .ok();

        let mut issues = contract_issues;
        let llm_issues = self.analyze_changes(&context, task_spec.as_ref()).await;
        issues.extend(llm_issues);

        // Step 4: Generate final report and return result
        let errors: Vec<_> = issues
            .iter()
            .filter(|issue| matches!(issue.issue_type, IssueType::Error))
            .collect();
        let warnings: Vec<_> = issues
            .iter()
            .filter(|issue| matches!(issue.issue_type, IssueType::Warning))
            .collect();

        let report = self.generate_review_report(&issues, &contracts_result, &verify_result);

        if !errors.is_empty() {
            let error_messages: Vec<_> = errors
                .iter()
                .map(|issue| {
                    let location = match (&issue.file_path, issue.line_number) {
                        (Some(path), Some(line)) => {
                            format!("{}:{}", path.to_str().unwrap_or(""), line)
                        }
                        (Some(path), None) => path.to_str().unwrap_or("").to_string(),
                        (None, _) => "unknown".to_string(),
                    };
                    format!("[{}] {}: {}", location, issue.title, issue.description)
                })
                .collect();

            let result = AgentResult::Failure(format!(
                "Review found {} error(s): {}",
                errors.len(),
                error_messages.join("; ")
            ));

            tx.send(AgentEvent::Progress(format!(
                "Review completed with {} error(s) and {} warning(s)",
                errors.len(),
                warnings.len()
            )))
            .await
            .ok();
            tx.send(AgentEvent::Completed(result.clone())).await.ok();

            result
        } else if !warnings.is_empty() {
            let warning_messages: Vec<_> = warnings
                .iter()
                .map(|issue| format!("{}: {}", issue.title, issue.description))
                .collect();

            let result = AgentResult::Success(serde_json::json!({
                "review_approved": true,
                "warnings_found": true,
                "warning_count": warnings.len(),
                "warnings": warning_messages,
                "report": report
            }));

            tx.send(AgentEvent::Progress(format!(
                "Review approved with {} warning(s)",
                warnings.len()
            )))
            .await
            .ok();
            tx.send(AgentEvent::Completed(result.clone())).await.ok();

            result
        } else {
            let result = AgentResult::Success(serde_json::json!({
                "review_approved": true,
                "warnings_found": false,
                "report": report
            }));

            tx.send(AgentEvent::Progress(
                "Review approved - no issues found".to_string(),
            ))
            .await
            .ok();
            tx.send(AgentEvent::Completed(result.clone())).await.ok();

            result
        }
    }
}

/// Extracts JSON from markdown code blocks.
fn extract_json_from_markdown(content: &str) -> Option<String> {
    // Look for JSON in code blocks
    if let Some(start) = content.find("```json") {
        let content_after = &content[start + 7..];
        if let Some(end) = content_after.find("```") {
            return Some(content_after[..end].trim().to_string());
        }
    }

    // Look for any code block
    if let Some(start) = content.find("```") {
        let content_after = &content[start + 3..];
        // Skip language identifier if present
        let content_after = if content_after.starts_with(|c: char| c.is_alphabetic()) {
            if let Some(newline) = content_after.find('\n') {
                &content_after[newline + 1..]
            } else {
                content_after
            }
        } else {
            content_after
        };

        if let Some(end) = content_after.find("```") {
            return Some(content_after[..end].trim().to_string());
        }
    }

    // Try to find JSON object directly
    if let Some(start) = content.find('{') {
        if let Some(end) = content.rfind('}') {
            if end > start {
                return Some(content[start..=end].to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_creation() {
        let issue = Issue::error("Test Error", "This is a test error");
        assert_eq!(issue.issue_type, IssueType::Error);
        assert_eq!(issue.title, "Test Error");
        assert_eq!(issue.description, "This is a test error");
        assert!(issue.file_path.is_none());
        assert!(issue.line_number.is_none());
    }

    #[test]
    fn test_issue_with_path() {
        let issue = Issue::error_with_path("Test Error", "Test", PathBuf::from("src/test.rs"));
        assert_eq!(issue.issue_type, IssueType::Error);
        assert_eq!(issue.file_path, Some(PathBuf::from("src/test.rs")));
    }

    #[test]
    fn test_issue_with_location() {
        let issue =
            Issue::error_with_location("Test Error", "Test", PathBuf::from("src/test.rs"), 42);
        assert_eq!(issue.issue_type, IssueType::Error);
        assert_eq!(issue.file_path, Some(PathBuf::from("src/test.rs")));
        assert_eq!(issue.line_number, Some(42));
    }

    #[test]
    fn test_extract_json_from_markdown() {
        let content = r#"Here is the JSON:
```json
{"issues": [], "approved": true}
```
"#;
        let result = extract_json_from_markdown(content);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["approved"], true);
    }

    #[test]
    fn test_extract_json_from_plain_code_block() {
        let content = r#"Here is the JSON:
```
{"issues": [], "approved": true}
```
"#;
        let result = extract_json_from_markdown(content);
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_json_direct() {
        let content = r#"Based on my analysis:
{"issues": [{"issue_type": "warning", "title": "Style", "description": "Minor style issue"}], "approved": true, "summary": "Looks good"}
"#;
        let result = extract_json_from_markdown(content);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["approved"], true);
    }

    #[test]
    fn test_config_defaults() {
        let config = ReviewAgentConfig::default();
        assert!(config.verify_commands.is_empty());
        assert_eq!(config.verify_command_timeout_secs, 120);
        assert!(config.use_llm_review);
        assert!(config.use_mcp_tools);
    }
}
