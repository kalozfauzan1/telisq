// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Unique identifier for a task in a plan.
pub type TaskId = String;

/// Unique identifier for a session.
pub type SessionId = Uuid;

/// Unique identifier for an agent instance.
pub type AgentId = String;

/// Unique identifier for an index entry.
pub type IndexId = String;

/// Path to a file in the project.
pub type FilePath = PathBuf;

/// Status of a task in a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task has not been started.
    Pending,
    /// Task is in progress.
    InProgress,
    /// Task has been completed successfully.
    Completed,
    /// Task has failed.
    Failed,
    /// Task has been skipped.
    Skipped,
}

/// Event types for TUI interaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TuiEvent {
    /// User has selected a task.
    TaskSelect(TaskId),
    /// User has started a task.
    TaskStart(TaskId),
    /// User has completed a task.
    TaskComplete(TaskId),
    /// User has failed a task.
    TaskFail(TaskId),
    /// User has skipped a task.
    TaskSkip(TaskId),
    /// User has paused a task.
    TaskPause(TaskId),
    /// User has resumed a task.
    TaskResume(TaskId),
    /// User has requested to view the task's details.
    TaskDetails(TaskId),
    /// User has requested to view the task's logs.
    TaskLogs(TaskId),
    /// User has requested to exit the TUI.
    Exit,
}

/// Specification for a task in a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Unique identifier for the task.
    pub id: TaskId,
    /// Human-readable title of the task.
    pub title: String,
    /// Optional description of the task.
    pub description: Option<String>,
    /// Status of the task.
    pub status: TaskStatus,
    /// List of task IDs that this task depends on.
    pub dependencies: Vec<TaskId>,
    /// List of files that this task affects.
    pub files: Vec<FilePath>,
    /// Optional list of contracts that this task must satisfy.
    pub contracts: Vec<String>,
}

impl TaskSpec {
    /// Creates a new task specification with default values.
    pub fn new(id: impl Into<TaskId>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            status: TaskStatus::Pending,
            dependencies: Vec::new(),
            files: Vec::new(),
            contracts: Vec::new(),
        }
    }

    /// Adds a dependency to the task.
    pub fn add_dependency(&mut self, task_id: impl Into<TaskId>) {
        self.dependencies.push(task_id.into());
    }

    /// Adds a file to the task's affected files list.
    pub fn add_file(&mut self, file: impl Into<FilePath>) {
        self.files.push(file.into());
    }

    /// Adds a contract to the task's contracts list.
    pub fn add_contract(&mut self, contract: impl Into<String>) {
        self.contracts.push(contract.into());
    }

    /// Sets the task's description.
    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = Some(description.into());
    }

    /// Sets the task's status.
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
    }
}

/// Represents a session in Telisq.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// Unique identifier for the session.
    pub id: SessionId,
    /// Name of the session.
    pub name: String,
    /// Path to the plan file being executed.
    pub plan_path: FilePath,
    /// Current state of the session.
    pub state: SessionState,
}

impl Session {
    /// Creates a new session with default values.
    pub fn new(name: impl Into<String>, plan_path: impl Into<FilePath>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            plan_path: plan_path.into(),
            state: SessionState::Running,
        }
    }
}

/// State of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Session is running normally.
    Running,
    /// Session has been paused.
    Paused,
    /// Session has been completed.
    Completed,
    /// Session has been canceled.
    Canceled,
}
