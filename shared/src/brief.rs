// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Agent type for dispatch routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    /// Plan agent for requirement analysis and plan generation.
    Plan,
    /// Code agent for code generation and patching.
    Code,
    /// Review agent for code review and validation.
    Review,
    /// Ask agent for user interaction and questions.
    Ask,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Plan => write!(f, "plan"),
            AgentType::Code => write!(f, "code"),
            AgentType::Review => write!(f, "review"),
            AgentType::Ask => write!(f, "ask"),
        }
    }
}

/// Brief sent from Plan Agent for work
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanBrief {
    /// The user's goal or requirement to plan for.
    pub goal: String,
    /// Optional context about the project.
    pub project_context: Option<String>,
    /// Optional constraints or preferences.
    pub constraints: Option<String>,
}

/// Brief sent from Code Agent for work
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeBrief {
    /// The task specification or description.
    pub task_spec: String,
    /// Files that this task will modify.
    pub files: Vec<PathBuf>,
    /// Optional plan context from the plan agent.
    pub plan_context: Option<String>,
    /// Optional contracts that must be satisfied.
    pub contracts: Vec<String>,
}

/// Brief sent from Review Agent for work
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBrief {
    /// The task specification to review against.
    pub task_spec: String,
    /// Files that were modified.
    pub changed_files: Vec<PathBuf>,
    /// Optional contracts to verify.
    pub contracts: Vec<String>,
}

/// Brief sent from Ask Agent for work
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskBrief {
    /// The question to ask the user.
    pub question: String,
    /// Optional options for the user to choose from.
    pub options: Option<Vec<String>>,
    /// Whether free text input is allowed.
    pub allow_free_text: bool,
    /// Optional context about the situation.
    pub context: Option<String>,
}

/// Brief sent from Telisq to Agent for work
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentBrief {
    /// Plan agent brief for requirement analysis.
    Plan(PlanBrief),
    /// Code agent brief for code generation.
    Code(CodeBrief),
    /// Review agent brief for code review.
    Review(ReviewBrief),
    /// Ask agent brief for user interaction.
    Ask(AskBrief),
    /// Create or modify files (legacy)
    Files { path: String, content: String },
    /// Execute a command (legacy)
    Shell {
        command: String,
        working_dir: Option<String>,
    },
    /// Ask user a question that requires yes/no response (legacy)
    Confirm { prompt: String },
    /// Get a value from the user (secret or not) (legacy)
    GetValue { prompt: String, secret: bool },
    /// Open an editor to write text or modify a file (legacy)
    OpenEditor {
        path: Option<String>,
        initial_text: Option<String>,
    },
    /// Open a browser to a specific URL (legacy)
    OpenBrowser { url: String },
    /// Request execution of an MCP function (legacy)
    McpFunction {
        server_name: String,
        procedure_name: String,
        params: serde_json::Value,
    },
    /// No more work to do in this task (legacy)
    Done,
}

impl AgentBrief {
    /// Creates a new Plan brief.
    pub fn plan(goal: impl Into<String>) -> Self {
        Self::Plan(PlanBrief {
            goal: goal.into(),
            project_context: None,
            constraints: None,
        })
    }

    /// Creates a new Code brief.
    pub fn code(task_spec: impl Into<String>) -> Self {
        Self::Code(CodeBrief {
            task_spec: task_spec.into(),
            files: Vec::new(),
            plan_context: None,
            contracts: Vec::new(),
        })
    }

    /// Creates a new Review brief.
    pub fn review(task_spec: impl Into<String>) -> Self {
        Self::Review(ReviewBrief {
            task_spec: task_spec.into(),
            changed_files: Vec::new(),
            contracts: Vec::new(),
        })
    }

    /// Creates a new Ask brief.
    pub fn ask(question: impl Into<String>) -> Self {
        Self::Ask(AskBrief {
            question: question.into(),
            options: None,
            allow_free_text: true,
            context: None,
        })
    }

    /// Creates a new Files brief for creating or modifying files.
    pub fn files(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Files {
            path: path.into(),
            content: content.into(),
        }
    }

    /// Creates a new Shell brief for executing a command.
    pub fn shell(command: impl Into<String>, working_dir: Option<impl Into<String>>) -> Self {
        Self::Shell {
            command: command.into(),
            working_dir: working_dir.map(|s| s.into()),
        }
    }

    /// Creates a new Confirm brief for asking yes/no questions.
    pub fn confirm(prompt: impl Into<String>) -> Self {
        Self::Confirm {
            prompt: prompt.into(),
        }
    }

    /// Creates a new GetValue brief for getting user input.
    pub fn get_value(prompt: impl Into<String>, secret: bool) -> Self {
        Self::GetValue {
            prompt: prompt.into(),
            secret,
        }
    }

    /// Creates a new OpenEditor brief for opening an editor.
    pub fn open_editor(
        path: Option<impl Into<String>>,
        initial_text: Option<impl Into<String>>,
    ) -> Self {
        Self::OpenEditor {
            path: path.map(|s| s.into()),
            initial_text: initial_text.map(|s| s.into()),
        }
    }

    /// Creates a new OpenBrowser brief for opening a browser.
    pub fn open_browser(url: impl Into<String>) -> Self {
        Self::OpenBrowser { url: url.into() }
    }

    /// Creates a new McpFunction brief for executing an MCP function.
    pub fn mcp_function(
        server_name: impl Into<String>,
        procedure_name: impl Into<String>,
        params: serde_json::Value,
    ) -> Self {
        Self::McpFunction {
            server_name: server_name.into(),
            procedure_name: procedure_name.into(),
            params,
        }
    }

    /// Creates a new Done brief indicating no more work.
    pub fn done() -> Self {
        Self::Done
    }

    /// Returns the agent type for this brief.
    pub fn agent_type(&self) -> AgentType {
        match self {
            AgentBrief::Plan(_) => AgentType::Plan,
            AgentBrief::Code(_) => AgentType::Code,
            AgentBrief::Review(_) => AgentType::Review,
            AgentBrief::Ask(_) => AgentType::Ask,
            _ => AgentType::Ask, // Legacy variants default to Ask
        }
    }

    /// Validates that the brief is complete and ready for dispatch.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            AgentBrief::Plan(brief) => {
                if brief.goal.is_empty() {
                    return Err("Plan brief goal cannot be empty".to_string());
                }
                Ok(())
            }
            AgentBrief::Code(brief) => {
                if brief.task_spec.is_empty() {
                    return Err("Code brief task_spec cannot be empty".to_string());
                }
                Ok(())
            }
            AgentBrief::Review(brief) => {
                if brief.task_spec.is_empty() {
                    return Err("Review brief task_spec cannot be empty".to_string());
                }
                Ok(())
            }
            AgentBrief::Ask(brief) => {
                if brief.question.is_empty() {
                    return Err("Ask brief question cannot be empty".to_string());
                }
                Ok(())
            }
            _ => Ok(()), // Legacy variants are always valid
        }
    }
}

/// Issue found during review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentIssue {
    /// Type of issue: error or warning.
    pub issue_type: String,
    /// Short description of the issue.
    pub title: String,
    /// Detailed description.
    pub description: String,
    /// File path where the issue was found.
    pub file_path: Option<String>,
}

/// Result sent from Agent to Telisq
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentResult {
    /// Agent completed successfully with result data.
    Success {
        /// Result data from the agent.
        data: serde_json::Value,
    },
    /// Agent failed to complete the task.
    Failed {
        /// Error message describing the failure.
        error: String,
    },
    /// Agent needs clarification from the user.
    NeedsClarification {
        /// The question that needs clarification.
        question: String,
        /// Optional context about why clarification is needed.
        context: Option<String>,
    },
    /// Review agent approved the changes.
    Approved {
        /// Optional summary of the review.
        review_summary: Option<String>,
    },
    /// Review agent found issues in the changes.
    IssuesFound {
        /// List of issues found during review.
        issues: Vec<AgentIssue>,
    },
    /// File has been created or modified (legacy)
    FileWritten { path: String },
    /// Command executed (legacy)
    CommandExecuted {
        command: String,
        working_dir: Option<String>,
        output: String,
        code: i32,
    },
    /// User confirmed (yes) (legacy)
    Confirmed,
    /// User rejected (no) (legacy)
    Rejected,
    /// User provided text input (legacy)
    TextValue { value: String },
    /// File was edited (legacy)
    FileEdited { path: String },
    /// Browser was opened (legacy)
    BrowserOpened { url: String },
    /// MCP function executed (legacy)
    McpResult { result: serde_json::Value },
    /// Agent needs more context about the plan (legacy)
    NeedPlanContext,
    /// Agent needs more context about the codebase (legacy)
    NeedCodebaseContext,
    /// Done with current context level (legacy)
    PopContext,
}

impl AgentResult {
    /// Creates a new Success result with data.
    pub fn success(data: serde_json::Value) -> Self {
        Self::Success { data }
    }

    /// Creates a new Failed result with error message.
    pub fn failed(error: impl Into<String>) -> Self {
        Self::Failed {
            error: error.into(),
        }
    }

    /// Creates a new NeedsClarification result.
    pub fn with_clarification_request(question: impl Into<String>) -> Self {
        Self::NeedsClarification {
            question: question.into(),
            context: None,
        }
    }

    /// Creates a new Approved result.
    pub fn approved() -> Self {
        Self::Approved {
            review_summary: None,
        }
    }

    /// Creates a new IssuesFound result.
    pub fn issues_found(issues: Vec<AgentIssue>) -> Self {
        Self::IssuesFound { issues }
    }

    /// Creates a new FileWritten result.
    pub fn file_written(path: impl Into<String>) -> Self {
        Self::FileWritten { path: path.into() }
    }

    /// Creates a new CommandExecuted result.
    pub fn command_executed(
        command: impl Into<String>,
        working_dir: Option<impl Into<String>>,
        output: impl Into<String>,
        code: i32,
    ) -> Self {
        Self::CommandExecuted {
            command: command.into(),
            working_dir: working_dir.map(|s| s.into()),
            output: output.into(),
            code,
        }
    }

    /// Creates a new Confirmed result.
    pub fn confirmed() -> Self {
        Self::Confirmed
    }

    /// Creates a new Rejected result.
    pub fn rejected() -> Self {
        Self::Rejected
    }

    /// Creates a new TextValue result.
    pub fn text_value(value: impl Into<String>) -> Self {
        Self::TextValue {
            value: value.into(),
        }
    }

    /// Creates a new FileEdited result.
    pub fn file_edited(path: impl Into<String>) -> Self {
        Self::FileEdited { path: path.into() }
    }

    /// Creates a new BrowserOpened result.
    pub fn browser_opened(url: impl Into<String>) -> Self {
        Self::BrowserOpened { url: url.into() }
    }

    /// Creates a new McpResult result.
    pub fn mcp_result(result: serde_json::Value) -> Self {
        Self::McpResult { result }
    }

    /// Creates a new NeedPlanContext result.
    pub fn need_plan_context() -> Self {
        Self::NeedPlanContext
    }

    /// Creates a new NeedCodebaseContext result.
    pub fn need_codebase_context() -> Self {
        Self::NeedCodebaseContext
    }

    /// Creates a new PopContext result.
    pub fn pop_context() -> Self {
        Self::PopContext
    }

    /// Returns true if this result indicates success.
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            AgentResult::Success { .. } | AgentResult::Approved { .. }
        )
    }

    /// Returns true if this result indicates failure.
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            AgentResult::Failed { .. } | AgentResult::IssuesFound { .. }
        )
    }

    /// Returns true if this result needs clarification.
    pub fn needs_clarification(&self) -> bool {
        matches!(self, AgentResult::NeedsClarification { .. })
    }
}
