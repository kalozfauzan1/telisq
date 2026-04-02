// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use crate::agents::ask_agent::{AskAgent, AskAgentConfig};
use crate::agents::code_agent::{CodeAgent, CodeAgentConfig};
use crate::agents::plan_agent::{PlanAgent, PlanAgentConfig};
use crate::agents::review_agent::{ReviewAgent, ReviewAgentConfig};
use crate::agents::*;
use crate::session::store::SessionStore;
use plan::graph::TaskGraph;
use plan::tracker::MarkerTracker;
use shared::brief::{AgentBrief, AgentResult as SharedAgentResult, AgentType, AskBrief, CodeBrief};
use shared::config::LlmConfig;
use shared::errors::Result;
use shared::types::{SessionId, TaskId, TaskSpec, TaskStatus};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::*;

/// Orchestrator configuration.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Whether to use a stronger model for orchestrator reasoning.
    pub use_strong_model: bool,
    /// Maximum number of clarification rounds.
    pub max_clarification_rounds: usize,
    /// Maximum number of retries for failed tasks.
    pub max_retries: usize,
    /// Whether to enable test-aware behavior.
    pub test_aware: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            use_strong_model: false,
            max_clarification_rounds: 3,
            max_retries: 3,
            test_aware: true,
        }
    }
}

/// Task information for orchestration.
#[derive(Debug)]
struct TaskInfo {
    /// Task specification.
    spec: TaskSpec,
    /// Agent runner for the task.
    agent: Box<dyn AgentRunner>,
    /// Result of the task (if completed).
    result: Option<SharedAgentResult>,
    /// Agent type for retry logic.
    agent_type: AgentType,
}

/// Events emitted by the orchestrator.
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// A task has started.
    StepStarted(TaskId),
    /// A task has completed successfully.
    StepCompleted(TaskId),
    /// A task has failed.
    StepFailed(TaskId, String),
    /// An agent has sent a message.
    AgentMessage(String),
    /// The entire plan has been completed.
    PlanCompleted,
    /// A plan marker has been updated in the plan file.
    PlanMarkerUpdated(TaskId, TaskStatus),
    /// Session has been stopped.
    SessionStopped(SessionId),
    /// A task retry attempt.
    TaskRetry(TaskId, usize, String),
}

/// Orchestrator for managing agents and task execution.
pub struct Orchestrator {
    config: OrchestratorConfig,
    /// Task storage.
    tasks: Arc<Mutex<HashMap<TaskId, TaskInfo>>>,
    /// Session identifier.
    session_id: SessionId,
    /// Event channel for communication with TUI.
    event_tx: Option<tokio::sync::mpsc::Sender<OrchestratorEvent>>,
    /// Optional LLM config for agent dispatch.
    llm_config: Option<LlmConfig>,
    /// Path to the plan file for marker updates.
    plan_path: Option<PathBuf>,
    /// Dependency graph for task execution ordering.
    task_graph: Option<Arc<Mutex<TaskGraph>>>,
    /// Task ID to resume from (continue_from option).
    continue_from: Option<TaskId>,
    /// Optional session store for persistence.
    session_store: Option<SessionStore>,
}

impl Orchestrator {
    /// Creates a new orchestrator instance.
    pub fn new(session_id: SessionId, config: Option<OrchestratorConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            session_id,
            event_tx: None,
            llm_config: None,
            plan_path: None,
            task_graph: None,
            continue_from: None,
            session_store: None,
        }
    }

    /// Creates a new orchestrator instance with LLM config.
    pub fn with_llm(
        session_id: SessionId,
        config: Option<OrchestratorConfig>,
        llm_config: Option<LlmConfig>,
    ) -> Self {
        Self {
            config: config.unwrap_or_default(),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            session_id,
            event_tx: None,
            llm_config,
            plan_path: None,
            task_graph: None,
            continue_from: None,
            session_store: None,
        }
    }

    /// Sets the plan path for marker updates.
    pub fn with_plan_path(mut self, path: PathBuf) -> Self {
        self.plan_path = Some(path);
        self
    }

    /// Sets the continue_from task ID for resuming execution.
    pub fn with_continue_from(mut self, task_id: TaskId) -> Self {
        self.continue_from = Some(task_id);
        self
    }

    /// Sets the session store for persistence.
    pub fn with_session_store(mut self, store: SessionStore) -> Self {
        self.session_store = Some(store);
        self
    }

    /// Initializes the task graph from task specs.
    pub fn init_task_graph(&mut self) -> Result<()> {
        let tasks = self.tasks.lock().unwrap();
        let task_specs: Vec<TaskSpec> = tasks.values().map(|t| t.spec.clone()).collect();
        drop(tasks);

        if task_specs.is_empty() {
            return Ok(());
        }

        let graph = TaskGraph::new(task_specs)?;
        graph.validate()?;
        self.task_graph = Some(Arc::new(Mutex::new(graph)));
        Ok(())
    }

    /// Sets the event channel for communication with TUI.
    pub fn set_event_tx(&mut self, tx: tokio::sync::mpsc::Sender<OrchestratorEvent>) {
        self.event_tx = Some(tx);
    }

    /// Sends an event to the TUI.
    fn send_event(&self, event: OrchestratorEvent) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.blocking_send(event);
        }
    }

    /// Updates the plan marker for a task and emits an event.
    fn update_plan_marker(&self, task_id: &TaskId, status: TaskStatus) {
        // Update in-memory task status
        {
            let mut tasks = self.tasks.lock().unwrap();
            if let Some(task_info) = tasks.get_mut(task_id) {
                task_info.spec.set_status(status);
            }
        }

        // Update plan file atomically
        if let Some(plan_path) = &self.plan_path {
            match MarkerTracker::update_marker(plan_path, task_id, status) {
                Ok(()) => {
                    info!(task_id = %task_id, ?status, "Updated plan marker");
                }
                Err(e) => {
                    warn!(task_id = %task_id, error = %e, "Failed to update plan marker");
                }
            }
        }

        // Emit event to TUI
        self.send_event(OrchestratorEvent::PlanMarkerUpdated(
            task_id.clone(),
            status,
        ));
    }

    /// Checks if all tasks are completed.
    fn all_tasks_done(&self) -> bool {
        let tasks = self.tasks.lock().unwrap();
        tasks
            .values()
            .all(|t| t.spec.status == TaskStatus::Completed)
    }

    /// Gets all changed files from completed tasks.
    fn get_changed_files(&self) -> Vec<std::path::PathBuf> {
        let tasks = self.tasks.lock().unwrap();
        tasks
            .values()
            .filter(|t| t.spec.status == TaskStatus::Completed)
            .flat_map(|t| t.spec.files.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Gets all contracts from completed tasks.
    fn get_all_contracts(&self) -> Vec<String> {
        let tasks = self.tasks.lock().unwrap();
        tasks
            .values()
            .filter(|t| t.spec.status == TaskStatus::Completed)
            .flat_map(|t| t.spec.contracts.clone())
            .collect()
    }

    /// Returns whether the orchestrator is currently running.
    pub fn is_running(&self) -> bool {
        let tasks = self.tasks.lock().unwrap();
        tasks
            .values()
            .any(|task_info| task_info.spec.status == TaskStatus::InProgress)
    }

    /// Adds a task to the orchestrator.
    pub fn add_task(
        &mut self,
        task_spec: TaskSpec,
        agent: Box<dyn AgentRunner>,
        agent_type: AgentType,
    ) {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(
            task_spec.id.clone(),
            TaskInfo {
                spec: task_spec,
                agent,
                result: None,
                agent_type,
            },
        );
    }

    /// Returns the current state of all tasks.
    pub fn get_tasks_state(&self) -> Vec<TaskSpec> {
        let tasks = self.tasks.lock().unwrap();
        tasks
            .values()
            .map(|task_info| task_info.spec.clone())
            .collect()
    }

    /// Finds all tasks that are ready to be executed.
    /// A task is ready if all its dependencies are completed.
    fn find_ready_tasks(&self) -> Vec<TaskId> {
        // If we have a task graph, use it for dependency-ordered execution
        if let Some(graph) = &self.task_graph {
            let graph = graph.lock().unwrap();
            let runnable = graph.get_runnable_tasks();
            return runnable.iter().map(|t| t.id.clone()).collect();
        }

        // Fallback to in-memory task checking
        let tasks = self.tasks.lock().unwrap();
        let mut ready_tasks = Vec::new();

        for (task_id, task_info) in tasks.iter() {
            if task_info.spec.status == TaskStatus::Pending {
                // If continue_from is set, only allow tasks at or after that point
                if let Some(continue_from) = &self.continue_from {
                    if task_id < continue_from {
                        continue;
                    }
                }

                let all_deps_completed = task_info.spec.dependencies.iter().all(|dep_id| {
                    tasks
                        .get(dep_id)
                        .is_some_and(|dep_info| dep_info.spec.status == TaskStatus::Completed)
                });

                if all_deps_completed {
                    ready_tasks.push(task_id.clone());
                }
            }
        }

        ready_tasks
    }

    /// Updates the status of a task.
    fn update_task_status(&self, task_id: &TaskId, status: TaskStatus) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task_info) = tasks.get_mut(task_id) {
            task_info.spec.set_status(status);
        }
    }

    /// Dispatches a brief to the appropriate agent type.
    ///
    /// This is the core agent-type dispatch function. It routes the brief to the
    /// correct agent (Plan, Code, Review, or Ask) based on the brief's agent type.
    /// Each invocation gets an isolated sub-session context.
    pub async fn dispatch_agent(
        &self,
        brief: AgentBrief,
        task_id: Option<TaskId>,
    ) -> SharedAgentResult {
        // Validate brief before dispatch
        if let Err(e) = brief.validate() {
            warn!(error = %e, "Brief validation failed");
            return SharedAgentResult::failed(format!("Brief validation failed: {}", e));
        }

        let agent_type = brief.agent_type();
        info!(?agent_type, "Dispatching to agent type");

        // Create isolated sub-session context
        let context = AgentContext::new(
            self.session_id,
            task_id.clone(),
            agent_type,
            self.config.max_retries,
            self.config.test_aware,
        );

        // Save sub-session ID for logging (context will be moved)
        let sub_session_id = context.sub_session_id;

        info!(
            sub_session_id = %sub_session_id,
            "Created isolated sub-session context"
        );

        // Route to appropriate agent based on agent type
        let result = match agent_type {
            AgentType::Plan => self.dispatch_plan_agent(brief, context).await,
            AgentType::Code => self.dispatch_code_agent(brief, context).await,
            AgentType::Review => self.dispatch_review_agent(brief, context).await,
            AgentType::Ask => self.dispatch_ask_agent(brief, context).await,
        };

        // Log result
        match &result {
            SharedAgentResult::Success { .. } => {
                info!(
                    sub_session_id = %sub_session_id,
                    "Agent completed successfully"
                );
            }
            SharedAgentResult::Failed { error } => {
                warn!(
                    sub_session_id = %sub_session_id,
                    error = error,
                    "Agent failed"
                );
            }
            _ => {
                info!(
                    sub_session_id = %sub_session_id,
                    "Agent completed with non-success result"
                );
            }
        }

        // Clean up sub-session resources (handled by dropping the context)
        info!(
            sub_session_id = %sub_session_id,
            "Cleaned up sub-session resources"
        );

        result
    }

    /// Converts local AgentResult to SharedAgentResult.
    fn convert_agent_result(result: AgentResult) -> SharedAgentResult {
        match result {
            AgentResult::Success(data) => SharedAgentResult::success(data),
            AgentResult::Failure(msg) => SharedAgentResult::failed(msg),
            AgentResult::UserInputRequired(msg) => {
                SharedAgentResult::with_clarification_request(msg)
            }
        }
    }

    /// Dispatches to PlanAgent.
    async fn dispatch_plan_agent(
        &self,
        _brief: AgentBrief,
        context: AgentContext,
    ) -> SharedAgentResult {
        let (tx, mut rx) = mpsc::channel(10);

        let plan_config = PlanAgentConfig {
            max_clarification_rounds: self.config.max_clarification_rounds,
            ..Default::default()
        };

        let agent = match &self.llm_config {
            Some(config) => PlanAgent::with_llm("plan_agent", Some(plan_config), config.clone()),
            None => PlanAgent::new("plan_agent", Some(plan_config), None, None, None, None),
        };

        // Spawn agent execution
        let task = tokio::spawn(async move { agent.run(context, tx).await });

        // Listen for events
        while let Some(event) = rx.recv().await {
            self.send_event(OrchestratorEvent::AgentMessage(format!(
                "Plan Agent: {:?}",
                event
            )));
        }

        // Wait for agent to complete
        match task.await {
            Ok(result) => Self::convert_agent_result(result),
            Err(e) => SharedAgentResult::failed(format!("Plan agent task failed: {}", e)),
        }
    }

    /// Dispatches to CodeAgent.
    async fn dispatch_code_agent(
        &self,
        _brief: AgentBrief,
        context: AgentContext,
    ) -> SharedAgentResult {
        let (tx, mut rx) = mpsc::channel(10);

        let code_config = CodeAgentConfig {
            max_retries: self.config.max_retries,
            test_aware: self.config.test_aware,
            ..Default::default()
        };

        let agent = match &self.llm_config {
            Some(config) => CodeAgent::with_llm("code_agent", Some(code_config), config.clone()),
            None => CodeAgent::new("code_agent", Some(code_config), None, None),
        };

        // Spawn agent execution
        let task = tokio::spawn(async move { agent.run(context, tx).await });

        // Listen for events
        while let Some(event) = rx.recv().await {
            self.send_event(OrchestratorEvent::AgentMessage(format!(
                "Code Agent: {:?}",
                event
            )));
        }

        // Wait for agent to complete
        match task.await {
            Ok(result) => Self::convert_agent_result(result),
            Err(e) => SharedAgentResult::failed(format!("Code agent task failed: {}", e)),
        }
    }

    /// Dispatches to ReviewAgent.
    async fn dispatch_review_agent(
        &self,
        _brief: AgentBrief,
        context: AgentContext,
    ) -> SharedAgentResult {
        let (tx, mut rx) = mpsc::channel(10);

        let review_config = ReviewAgentConfig {
            ..Default::default()
        };

        let agent = match &self.llm_config {
            Some(config) => {
                ReviewAgent::with_llm("review_agent", Some(review_config), config.clone())
            }
            None => ReviewAgent::new("review_agent", Some(review_config), None, None),
        };

        // Spawn agent execution
        let task = tokio::spawn(async move { agent.run(context, tx).await });

        // Listen for events
        while let Some(event) = rx.recv().await {
            self.send_event(OrchestratorEvent::AgentMessage(format!(
                "Review Agent: {:?}",
                event
            )));
        }

        // Wait for agent to complete
        match task.await {
            Ok(result) => Self::convert_agent_result(result),
            Err(e) => SharedAgentResult::failed(format!("Review agent task failed: {}", e)),
        }
    }

    /// Dispatches to AskAgent.
    async fn dispatch_ask_agent(
        &self,
        _brief: AgentBrief,
        context: AgentContext,
    ) -> SharedAgentResult {
        let (tx, mut rx) = mpsc::channel(10);

        let ask_config = AskAgentConfig {
            input_timeout: 300,
            allow_free_text: true,
            use_llm: self.llm_config.is_some(),
        };

        let agent = match &self.llm_config {
            Some(config) => AskAgent::with_llm("ask_agent", Some(ask_config), config.clone()),
            None => AskAgent::new("ask_agent", Some(ask_config), None),
        };

        // Spawn agent execution
        let task = tokio::spawn(async move { agent.run(context, tx).await });

        // Listen for events
        while let Some(event) = rx.recv().await {
            match event {
                AgentEvent::Started => {
                    self.send_event(OrchestratorEvent::AgentMessage(
                        "Ask Agent: Started".to_string(),
                    ));
                }
                AgentEvent::Progress(msg) => {
                    self.send_event(OrchestratorEvent::AgentMessage(format!(
                        "Ask Agent: {}",
                        msg
                    )));
                }
                AgentEvent::Completed(result) => {
                    self.send_event(OrchestratorEvent::AgentMessage(format!(
                        "Ask Agent: Completed - {:?}",
                        result
                    )));
                }
                AgentEvent::UserInputRequired {
                    question,
                    options: _,
                    answer_tx,
                } => {
                    self.send_event(OrchestratorEvent::AgentMessage(format!(
                        "Ask Agent: UserInputRequired - {}",
                        question
                    )));

                    // In a full TUI integration, this would trigger the TUI to:
                    // 1. Switch to AskAgentInput mode
                    // 2. Display the question and options
                    // 3. Wait for user input
                    // 4. Send the response back via answer_tx
                    //
                    // For now, we'll send a default response
                    let _ = answer_tx.send("skip".to_string());
                }
            }
        }

        // Wait for agent to complete
        match task.await {
            Ok(result) => Self::convert_agent_result(result),
            Err(e) => SharedAgentResult::failed(format!("Ask agent task failed: {}", e)),
        }
    }

    /// Runs a single task using the legacy agent runner.
    /// For Code agents, implements retry logic with error feedback.
    async fn run_task(&self, task_id: &TaskId) -> AgentResult {
        let task_info = {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.remove(task_id)
        };

        match task_info {
            Some(task_info) => {
                let agent_type = task_info.agent_type.clone();

                // Update marker to in_progress
                self.update_plan_marker(task_id, TaskStatus::InProgress);
                self.send_event(OrchestratorEvent::StepStarted(task_id.clone()));

                // Clone spec and agent type before moving agent
                let spec_clone = task_info.spec.clone();
                let agent = task_info.agent;

                // For Code agents, implement retry logic
                if agent_type == AgentType::Code {
                    let result = self.run_code_agent_with_retries(task_id, agent).await;

                    // Update task result and status based on result
                    {
                        let mut tasks = self.tasks.lock().unwrap();
                        let mut spec = spec_clone;
                        let new_status = match &result {
                            AgentResult::Success(_) => TaskStatus::Completed,
                            _ => TaskStatus::Failed,
                        };
                        spec.set_status(new_status);
                        tasks.insert(
                            task_id.clone(),
                            TaskInfo {
                                spec,
                                agent: Box::new(DummyAgent::new("completed")),
                                result: Some(match &result {
                                    AgentResult::Success(data) => {
                                        SharedAgentResult::Success { data: data.clone() }
                                    }
                                    AgentResult::Failure(msg) => {
                                        SharedAgentResult::Failed { error: msg.clone() }
                                    }
                                    _ => SharedAgentResult::Failed {
                                        error: "Unknown error".to_string(),
                                    },
                                }),
                                agent_type,
                            },
                        );
                    }

                    // Update marker based on result
                    let new_status = match &result {
                        AgentResult::Success(_) => TaskStatus::Completed,
                        _ => TaskStatus::Failed,
                    };
                    self.update_plan_marker(task_id, new_status);

                    // Send event
                    match &result {
                        AgentResult::Success(_) => {
                            self.send_event(OrchestratorEvent::StepCompleted(task_id.clone()))
                        }
                        AgentResult::Failure(msg) => self.send_event(
                            OrchestratorEvent::StepFailed(task_id.clone(), msg.clone()),
                        ),
                        _ => self.send_event(OrchestratorEvent::StepFailed(
                            task_id.clone(),
                            "Unknown error".to_string(),
                        )),
                    }

                    return result;
                }

                // Non-Code agents: run normally
                let (tx, mut rx) = mpsc::channel(10);
                let context = AgentContext::new(
                    self.session_id,
                    Some(task_id.clone()),
                    agent_type,
                    self.config.max_retries,
                    self.config.test_aware,
                );

                // Spawn task execution
                let task_id_clone = task_id.clone();
                let task_handle = tokio::spawn(async move {
                    let result = agent.run(context, tx).await;
                    (task_id_clone, result)
                });

                // Listen for events
                while let Some(event) = rx.recv().await {
                    self.send_event(OrchestratorEvent::AgentMessage(format!(
                        "Task {}: {:?}",
                        task_id, event
                    )));
                }

                // Wait for task to complete
                let (task_id, result) = match task_handle.await {
                    Ok(r) => r,
                    Err(e) => {
                        return AgentResult::Failure(format!("Task panicked: {}", e));
                    }
                };

                let new_status = match &result {
                    AgentResult::Success(_) => TaskStatus::Completed,
                    AgentResult::Failure(_) => TaskStatus::Failed,
                    _ => TaskStatus::Failed,
                };

                // Update marker based on result
                self.update_plan_marker(&task_id, new_status);

                // Update task result and status
                {
                    let mut tasks = self.tasks.lock().unwrap();
                    let mut spec = spec_clone;
                    spec.set_status(new_status);
                    tasks.insert(
                        task_id.clone(),
                        TaskInfo {
                            spec,
                            agent: Box::new(DummyAgent::new("completed")),
                            result: Some(SharedAgentResult::Success {
                                data: serde_json::json!({ "message": "Task completed" }),
                            }),
                            agent_type,
                        },
                    );
                }

                match &result {
                    AgentResult::Success(_) => {
                        self.send_event(OrchestratorEvent::StepCompleted(task_id.clone()))
                    }
                    AgentResult::Failure(msg) => {
                        self.send_event(OrchestratorEvent::StepFailed(task_id.clone(), msg.clone()))
                    }
                    _ => self.send_event(OrchestratorEvent::StepFailed(
                        task_id.clone(),
                        "Unknown error".to_string(),
                    )),
                }

                result
            }
            None => AgentResult::Failure(format!("Task not found: {}", task_id)),
        }
    }

    /// Runs a Code Agent with retry logic.
    ///
    /// On failure, retries up to `max_retries` times with error feedback.
    /// Each retry includes the previous error message for context.
    async fn run_code_agent_with_retries(
        &self,
        task_id: &TaskId,
        agent: Box<dyn AgentRunner>,
    ) -> AgentResult {
        let max_retries = self.config.max_retries;
        let mut last_error: Option<String> = None;

        // Wrap agent in Arc for sharing across retries
        let agent = Arc::new(agent);

        for attempt in 1..=max_retries {
            if attempt > 1 {
                info!(
                    task_id = %task_id,
                    attempt = attempt,
                    max_retries = max_retries,
                    previous_error = ?last_error,
                    "Retrying Code Agent"
                );
                self.send_event(OrchestratorEvent::TaskRetry(
                    task_id.clone(),
                    attempt,
                    last_error.clone().unwrap_or_default(),
                ));
            }

            // Create context with previous error feedback for retry
            let mut context = AgentContext::new(
                self.session_id,
                Some(task_id.clone()),
                AgentType::Code,
                self.config.max_retries,
                self.config.test_aware,
            );

            if let Some(ref error) = last_error {
                context.set_metadata(
                    "previous_error".to_string(),
                    format!("Previous attempt failed with: {}", error),
                );
            }

            let (tx, mut rx) = mpsc::channel(10);

            // Clone agent Arc for this attempt
            let agent_clone = Arc::clone(&agent);

            // Spawn task execution
            let task_id_clone = task_id.clone();
            let task_handle = tokio::spawn(async move {
                let result = agent_clone.run(context, tx).await;
                (task_id_clone, result)
            });

            // Listen for events
            while let Some(event) = rx.recv().await {
                self.send_event(OrchestratorEvent::AgentMessage(format!(
                    "Task {} (attempt {}): {:?}",
                    task_id, attempt, event
                )));
            }

            // Wait for task to complete
            let (_, result) = match task_handle.await {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(format!("Task panicked: {}", e));
                    continue;
                }
            };

            match &result {
                AgentResult::Success(_) => {
                    info!(task_id = %task_id, attempt = attempt, "Code Agent succeeded");
                    return result;
                }
                AgentResult::Failure(error_msg) => {
                    warn!(
                        task_id = %task_id,
                        attempt = attempt,
                        error = %error_msg,
                        "Code Agent failed"
                    );
                    last_error = Some(error_msg.clone());
                }
                _ => {
                    info!(task_id = %task_id, "Code Agent returned non-success result");
                    return result;
                }
            }
        }

        // All retries exhausted
        warn!(
            task_id = %task_id,
            max_retries = max_retries,
            "All Code Agent retries exhausted"
        );

        AgentResult::Failure(format!(
            "Code Agent failed after {} attempts: {}",
            max_retries,
            last_error.unwrap_or_default()
        ))
    }

    /// Asks the user what to do after max retries are exhausted.
    ///
    /// Options: retry again, skip task, stop session.
    async fn ask_user_after_max_retries(&self, task_id: &TaskId, error: &str) -> SharedAgentResult {
        let question = format!(
            "Task '{}' failed.\nError: {}\n\nWhat would you like to do?",
            task_id, error
        );

        let brief = AgentBrief::Ask(AskBrief {
            question,
            options: Some(vec![
                "retry".to_string(),
                "skip".to_string(),
                "stop".to_string(),
            ]),
            allow_free_text: false,
            context: Some(format!("Task: {}, Error: {}", task_id, error)),
        });

        self.dispatch_agent(brief, Some(task_id.clone())).await
    }

    /// Handles user decision after task failure.
    ///
    /// Returns the decision string: "retry", "skip", "stop", or None.
    async fn handle_user_decision(&self, task_id: &TaskId, error: &str) -> Option<String> {
        let result = self.ask_user_after_max_retries(task_id, error).await;

        match result {
            SharedAgentResult::Success { data } => data
                .get("answer")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            _ => None,
        }
    }

    /// Skips a task and marks it as skipped.
    fn skip_task(&self, task_id: &TaskId) {
        info!(task_id = %task_id, "Skipping task");
        self.update_plan_marker(task_id, TaskStatus::Skipped);

        // Cascade skip to dependent tasks
        self.mark_dependents_skippable(task_id);
    }

    /// Marks tasks that depend on a skipped task as skippable (prompts user).
    fn mark_dependents_skippable(&self, skipped_task_id: &TaskId) {
        let tasks_to_prompt: Vec<TaskId> = {
            let tasks = self.tasks.lock().unwrap();
            tasks
                .iter()
                .filter(|(_, info)| {
                    info.spec.dependencies.contains(skipped_task_id)
                        && info.spec.status == TaskStatus::Pending
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        for task_id in tasks_to_prompt {
            info!(
                task_id = %task_id,
                blocked_by = %skipped_task_id,
                "Task is blocked by skipped dependency"
            );
            // Mark as skipped automatically to prevent deadlock
            self.update_plan_marker(&task_id, TaskStatus::Skipped);
        }
    }

    /// Stops the session: persists state, resets in-progress markers, emits event.
    async fn stop_session(&self) {
        info!(session_id = %self.session_id, "Stopping session");

        // Reset in-progress tasks to pending
        {
            let mut tasks = self.tasks.lock().unwrap();
            for (_, info) in tasks.iter_mut() {
                if info.spec.status == TaskStatus::InProgress {
                    info.spec.set_status(TaskStatus::Pending);
                }
            }
        }

        // Persist session state if store is available
        if let Some(store) = &self.session_store {
            if let Err(e) = store
                .update_session_status(self.session_id, "stopped")
                .await
            {
                warn!(error = %e, "Failed to update session status");
            }
        }

        // Emit SessionStopped event
        self.send_event(OrchestratorEvent::SessionStopped(self.session_id.clone()));
    }

    /// Resumes a session from the session store.
    ///
    /// Loads session state, restores task markers and agent results,
    /// resets in-progress tasks to pending, and returns the continue_from task ID.
    pub async fn resume_from_store(&mut self) -> Result<Option<TaskId>> {
        let store = match &self.session_store {
            Some(s) => s,
            None => {
                warn!("No session store configured, cannot resume");
                return Ok(None);
            }
        };

        info!(session_id = %self.session_id, "Resuming session from store");

        // Load session
        let session = match store
            .load_session(self.session_id.clone())
            .await
            .map_err(|e| {
                shared::errors::TelisqError::Session(shared::errors::SessionError::LoadError(
                    e.to_string(),
                ))
            })? {
            Some(s) => s,
            None => {
                warn!(session_id = %self.session_id, "Session not found");
                return Ok(None);
            }
        };

        info!(
            session_id = %self.session_id,
            session_name = %session.name,
            "Session loaded"
        );

        // Load plan markers
        let markers = store
            .load_plan_markers(self.session_id.clone())
            .await
            .map_err(|e| {
                shared::errors::TelisqError::Session(shared::errors::SessionError::LoadError(
                    e.to_string(),
                ))
            })?;
        info!(marker_count = markers.len(), "Plan markers loaded");

        // Load agent results
        let results = store
            .load_agent_results(self.session_id.clone())
            .await
            .map_err(|e| {
                shared::errors::TelisqError::Session(shared::errors::SessionError::LoadError(
                    e.to_string(),
                ))
            })?;
        info!(result_count = results.len(), "Agent results loaded");

        // Restore task states from markers
        {
            let mut tasks = self.tasks.lock().unwrap();
            for (task_id, marker) in &markers {
                if let Some(info) = tasks.get_mut(task_id) {
                    let status = match marker.as_str() {
                        "completed" | "done" => TaskStatus::Completed,
                        "failed" => TaskStatus::Failed,
                        "skipped" => TaskStatus::Skipped,
                        "in_progress" => TaskStatus::Pending, // Reset in-progress to pending
                        _ => TaskStatus::Pending,
                    };
                    info.spec.set_status(status);
                }
            }
        }

        // Restore agent results to tasks
        {
            let mut tasks = self.tasks.lock().unwrap();
            for (task_id, result) in &results {
                if let Some(info) = tasks.get_mut(task_id) {
                    info.result = Some(SharedAgentResult::Success {
                        data: result.clone(),
                    });
                }
            }
        }

        // Find the last in-progress task to resume from
        let resume_from = {
            let tasks = self.tasks.lock().unwrap();
            tasks
                .iter()
                .find(|(_, info)| info.spec.status == TaskStatus::Pending)
                .map(|(id, _)| id.clone())
        };

        if let Some(ref task_id) = resume_from {
            info!(task_id = %task_id, "Resuming from task");
            self.continue_from = resume_from.clone();
        }

        Ok(resume_from)
    }

    /// Runs all tasks in dependency order.
    pub async fn run(&self) -> Result<()> {
        self.send_event(OrchestratorEvent::AgentMessage(format!(
            "Starting orchestration for session: {}",
            self.session_id
        )));

        let mut completed_tasks = HashSet::new();
        let mut failed_tasks = HashSet::new();
        let skipped_tasks: HashSet<TaskId> = HashSet::new();

        // Main task loop
        loop {
            // Find ready tasks
            let ready_tasks = self.find_ready_tasks();
            if ready_tasks.is_empty() {
                // Check if all tasks are completed
                if self.all_tasks_done() {
                    info!("All tasks completed, triggering review agent");
                    // Auto-trigger review agent
                    match self.run_review_agent().await {
                        SharedAgentResult::Success { .. } | SharedAgentResult::Approved { .. } => {
                            info!("Review agent approved changes");
                            self.send_event(OrchestratorEvent::AgentMessage(
                                "Review approved - session complete".to_string(),
                            ));
                            break;
                        }
                        SharedAgentResult::IssuesFound { issues } => {
                            warn!(?issues, "Review agent found issues");
                            // Prompt user via Ask Agent
                            let user_decision = self.ask_review_decision(&issues).await;
                            match user_decision.as_deref() {
                                Some("fix") => {
                                    info!("User chose to fix issues");
                                    // Spawn Code Agent for each blocking issue
                                    self.fix_review_issues(&issues).await;
                                }
                                Some("accept") => {
                                    info!("User chose to accept issues");
                                    break;
                                }
                                _ => {
                                    info!("No decision made, stopping");
                                    break;
                                }
                            }
                        }
                        SharedAgentResult::Failed { error } => {
                            warn!(error = %error, "Review agent failed");
                            break;
                        }
                        _ => {
                            info!("Review agent returned unexpected result");
                            break;
                        }
                    }
                }

                // If there are no ready tasks but tasks are still pending, check for deadlocks
                info!("No ready tasks. Checking for possible deadlocks...");
                let has_incomplete = {
                    let tasks = self.tasks.lock().unwrap();
                    tasks.values().any(|t| {
                        t.spec.status != TaskStatus::Completed
                            && t.spec.status != TaskStatus::Failed
                            && t.spec.status != TaskStatus::Skipped
                    })
                };

                if has_incomplete {
                    // Deadlock detected - prompt user via Ask Agent
                    warn!(
                        "Possible deadlock detected: no runnable tasks but incomplete tasks remain"
                    );
                    let user_decision = self.ask_deadlock_resolution().await;
                    match user_decision.as_deref() {
                        Some("continue") => {
                            info!("User chose to continue");
                            continue;
                        }
                        Some("skip") => {
                            info!("User chose to skip blocked tasks");
                            // Skip tasks that are blocked by failed/skipped deps
                            self.skip_blocked_tasks();
                            continue;
                        }
                        Some("stop") => {
                            info!("User chose to stop execution");
                            break;
                        }
                        _ => {
                            info!("No decision made, stopping");
                            break;
                        }
                    }
                }

                break;
            }

            // Run each ready task
            for task_id in ready_tasks {
                info!("Running task: {}", task_id);

                let result = self.run_task(&task_id).await;
                match result {
                    AgentResult::Success(_) => {
                        info!("Task {} completed successfully", task_id);
                        completed_tasks.insert(task_id);
                    }
                    AgentResult::Failure(error_msg) => {
                        warn!(task_id = %task_id, error = %error_msg, "Task failed");
                        failed_tasks.insert(task_id.clone());

                        // Handle user decision: skip or stop
                        let decision = self.handle_user_decision(&task_id, &error_msg).await;
                        match decision.as_deref() {
                            Some("skip") => {
                                info!(task_id = %task_id, "User chose to skip task");
                                self.skip_task(&task_id);
                                // Continue to next task
                            }
                            Some("stop") => {
                                info!(task_id = %task_id, "User chose to stop session");
                                self.stop_session().await;
                                return Ok(());
                            }
                            Some("retry") => {
                                info!(task_id = %task_id, "User chose to retry");
                                // Reset task to pending for retry
                                self.update_task_status(&task_id, TaskStatus::Pending);
                                // Will be picked up in next iteration
                            }
                            _ => {
                                info!(task_id = %task_id, "No valid decision, skipping task");
                                self.skip_task(&task_id);
                            }
                        }
                    }
                    AgentResult::UserInputRequired(msg) => {
                        info!(task_id = %task_id, "Task requires user input: {}", msg);
                        // Handle via Ask Agent in run_task
                    }
                }
            }
        }

        self.send_event(OrchestratorEvent::AgentMessage(format!(
            "Orchestration completed. Completed tasks: {}, Failed tasks: {}, Skipped tasks: {}",
            completed_tasks.len(),
            failed_tasks.len(),
            skipped_tasks.len()
        )));
        self.send_event(OrchestratorEvent::PlanCompleted);

        Ok(())
    }

    /// Runs the review agent after all tasks are completed.
    async fn run_review_agent(&self) -> SharedAgentResult {
        let changed_files = self.get_changed_files();
        let contracts = self.get_all_contracts();

        info!(
            changed_files_count = changed_files.len(),
            "Running review agent"
        );

        // Create a review brief
        let mut brief = AgentBrief::review("Review all completed tasks");
        if let AgentBrief::Review(ref mut review_brief) = brief {
            review_brief.changed_files = changed_files;
            review_brief.contracts = contracts;
        }

        // Dispatch to review agent
        self.dispatch_agent(brief, None).await
    }

    /// Asks the user how to handle review issues.
    async fn ask_review_decision(&self, issues: &[shared::brief::AgentIssue]) -> Option<String> {
        let issue_count = issues.len();
        let blocking_count = issues.iter().filter(|i| i.issue_type == "error").count();

        let question = format!(
            "Review found {} issues ({} blocking). How would you like to proceed?",
            issue_count, blocking_count
        );

        let brief = AgentBrief::Ask(AskBrief {
            question,
            options: Some(vec!["fix".to_string(), "accept".to_string()]),
            allow_free_text: false,
            context: Some(format!("Issues: {:?}", issues)),
        });

        let result = self.dispatch_agent(brief, None).await;
        match result {
            SharedAgentResult::Success { data } => data
                .get("answer")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            _ => None,
        }
    }

    /// Asks the user how to handle deadlock resolution.
    async fn ask_deadlock_resolution(&self) -> Option<String> {
        let brief = AgentBrief::Ask(AskBrief {
            question: "No runnable tasks but incomplete tasks remain. Possible deadlock. What would you like to do?".to_string(),
            options: Some(vec!["continue".to_string(), "skip".to_string(), "stop".to_string()]),
            allow_free_text: false,
            context: None,
        });

        let result = self.dispatch_agent(brief, None).await;
        match result {
            SharedAgentResult::Success { data } => data
                .get("answer")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            _ => None,
        }
    }

    /// Spawns Code Agent to fix issues found by review agent.
    async fn fix_review_issues(&self, issues: &[shared::brief::AgentIssue]) {
        let blocking_issues: Vec<_> = issues.iter().filter(|i| i.issue_type == "error").collect();

        for issue in blocking_issues {
            info!(?issue, "Fixing review issue");

            let task_spec = issue
                .file_path
                .as_ref()
                .map(|p| format!("Fix issue in {}: {}", p, issue.title))
                .unwrap_or_else(|| format!("Fix issue: {}", issue.title));

            let brief = AgentBrief::Code(CodeBrief {
                task_spec,
                files: issue
                    .file_path
                    .as_ref()
                    .map(|p| vec![PathBuf::from(p)])
                    .unwrap_or_default(),
                plan_context: Some(issue.description.clone()),
                contracts: vec![],
            });

            let result = self.dispatch_agent(brief, None).await;
            match result {
                SharedAgentResult::Success { .. } => {
                    info!("Successfully fixed issue: {}", issue.title);
                }
                SharedAgentResult::Failed { error } => {
                    warn!(error = %error, "Failed to fix issue: {}", issue.title);
                }
                _ => {}
            }
        }
    }

    /// Skips tasks that are blocked by failed/skipped dependencies.
    fn skip_blocked_tasks(&self) {
        loop {
            let mut to_skip = Vec::new();
            {
                let tasks = self.tasks.lock().unwrap();
                for (task_id, task_info) in tasks.iter() {
                    if task_info.spec.status == TaskStatus::Pending {
                        let has_blocked_dep = task_info.spec.dependencies.iter().any(|dep_id| {
                            tasks.get(dep_id).is_some_and(|dep| {
                                dep.spec.status == TaskStatus::Failed
                                    || dep.spec.status == TaskStatus::Skipped
                            })
                        });

                        if has_blocked_dep {
                            to_skip.push(task_id.clone());
                        }
                    }
                }
            }

            if to_skip.is_empty() {
                break;
            }

            let mut tasks = self.tasks.lock().unwrap();
            for task_id in to_skip {
                if let Some(task_info) = tasks.get_mut(&task_id) {
                    task_info.spec.set_status(TaskStatus::Skipped);
                }
            }
            drop(tasks);
        }
    }

    /// Gracefully shuts down the orchestrator.
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down orchestrator...");
        // TODO: Implement shutdown logic
        Ok(())
    }
}

impl Drop for Orchestrator {
    fn drop(&mut self) {
        // TODO: Cleanup resources on drop
    }
}

/// Simple dummy agent for placeholder use.
pub struct DummyAgent {
    id: AgentId,
}

impl DummyAgent {
    /// Creates a new dummy agent with the given id.
    pub fn new(id: impl Into<AgentId>) -> Self {
        Self { id: id.into() }
    }
}

impl std::fmt::Debug for DummyAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DummyAgent({})", self.id)
    }
}

#[async_trait::async_trait]
impl AgentRunner for DummyAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
    }

    async fn run(
        &self,
        context: AgentContext,
        tx: tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> AgentResult {
        tx.send(AgentEvent::Started).await.ok();
        tx.send(AgentEvent::Progress("Running dummy agent".to_string()))
            .await
            .ok();

        // Simulate work
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let result = AgentResult::Success(serde_json::json!({
            "dummy_result": "success",
            "task_id": context.task_id,
            "sub_session_id": context.sub_session_id.to_string(),
        }));

        tx.send(AgentEvent::Completed(result.clone())).await.ok();

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::types::TaskSpec;

    #[tokio::test]
    async fn test_orchestrator_simple_flow() {
        let session_id = shared::types::SessionId::new_v4();
        let mut orchestrator = Orchestrator::new(session_id, None);

        // Create a simple task
        let task_spec = TaskSpec::new("test-task", "Test Task");

        // Create a dummy agent
        let dummy_agent = Box::new(DummyAgent::new("dummy_agent"));

        // Add task to orchestrator
        orchestrator.add_task(task_spec, dummy_agent, AgentType::Plan);

        // Run orchestrator
        let result = orchestrator.run().await;

        // Should not return an error
        assert!(result.is_ok());
    }

    #[test]
    fn test_agent_brief_validation() {
        // Valid plan brief
        let plan_brief = AgentBrief::plan("Create a new feature");
        assert!(plan_brief.validate().is_ok());
        assert_eq!(plan_brief.agent_type(), AgentType::Plan);

        // Valid code brief
        let code_brief = AgentBrief::code("Implement the feature");
        assert!(code_brief.validate().is_ok());
        assert_eq!(code_brief.agent_type(), AgentType::Code);

        // Valid review brief
        let review_brief = AgentBrief::review("Review the changes");
        assert!(review_brief.validate().is_ok());
        assert_eq!(review_brief.agent_type(), AgentType::Review);

        // Valid ask brief
        let ask_brief = AgentBrief::ask("What should we do?");
        assert!(ask_brief.validate().is_ok());
        assert_eq!(ask_brief.agent_type(), AgentType::Ask);

        // Invalid brief (empty goal)
        let invalid_brief = AgentBrief::plan("");
        assert!(invalid_brief.validate().is_err());
    }

    #[test]
    fn test_agent_result_helpers() {
        let success = SharedAgentResult::success(serde_json::json!({"key": "value"}));
        assert!(success.is_success());

        let failed = SharedAgentResult::failed("Something went wrong");
        assert!(failed.is_failure());

        let clarification = SharedAgentResult::with_clarification_request("Need more info");
        assert!(clarification.needs_clarification());

        let approved = SharedAgentResult::approved();
        assert!(approved.is_success());
    }
}
