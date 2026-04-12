// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use crate::tui::components;
use crate::tui::events::{Event, Events};
use crate::tui::{start_tui, stop_tui};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;
use ratatui::Terminal;
use shared::types::{SessionId, TaskId, TaskSpec, TaskStatus};
use shared::types::SessionState as SharedSessionState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use telisq_core::orchestrator::{Orchestrator, OrchestratorEvent};
use telisq_core::session::store::SessionStore;
use tokio::runtime::Handle;
use tracing::{warn, info};

/// Application mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Normal operation mode.
    Normal,
    /// Asking for user input mode.
    AskAgentInput,
    /// Command input mode for dashboard chat.
    CommandInput,
}

/// Task display information for the session view.
#[derive(Debug, Clone)]
pub struct TaskDisplay {
    pub id: TaskId,
    pub title: String,
    pub status: TaskStatus,
}

/// Agent activity log entry.
#[derive(Debug, Clone)]
pub struct AgentLogEntry {
    pub timestamp: Instant,
    pub message: String,
}

/// Index status information.
#[derive(Debug, Clone, Default)]
pub struct IndexStatus {
    pub ollama_connected: bool,
    pub qdrant_connected: bool,
    pub indexed_file_count: usize,
    pub last_update: Option<String>,
    pub is_indexing: bool,
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub show_search: bool,
}

/// Search result from the index.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub score: f32,
    pub content_preview: String,
}

/// Chat message in the command input history.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatRole {
    User,
    Agent,
    System,
}

/// Application state.
#[derive(Debug)]
pub struct AppState {
    pub mode: AppMode,
    pub current_step: Option<String>,
    pub plan_nodes: Vec<String>,
    pub agent_messages: Vec<String>,
    pub session_id: Option<String>,
    pub quit_confirm: bool,
    pub active_panel: ActivePanel,
    /// Current text input buffer for AskAgentInput mode.
    pub input_buffer: String,
    /// Question to display in AskAgentInput mode.
    pub ask_question: Option<String>,
    /// Options to display in AskAgentInput mode.
    pub ask_options: Vec<String>,
    /// Task list for session view.
    pub tasks: Vec<TaskDisplay>,
    /// Agent activity log.
    pub agent_log: Vec<AgentLogEntry>,
    /// Overall session progress (0-100).
    pub session_progress: u16,
    /// Session status string.
    pub session_status: String,
    /// Index status.
    pub index_status: IndexStatus,
    /// Plan path if available.
    pub plan_path: Option<String>,
    /// Task status counts.
    pub task_counts: HashMap<TaskStatus, usize>,
    /// Chat message history for command input mode.
    pub chat_messages: Vec<ChatMessage>,
    /// Command history for up/down arrow navigation.
    pub command_history: Vec<String>,
    /// Current position in command history.
    pub history_index: usize,
}

/// Active panel in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Plan,
    Session,
    Agent,
    Index,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mode: AppMode::Normal,
            current_step: None,
            plan_nodes: Vec::new(),
            agent_messages: Vec::new(),
            session_id: None,
            quit_confirm: false,
            active_panel: ActivePanel::Plan,
            input_buffer: String::new(),
            ask_question: None,
            ask_options: Vec::new(),
            tasks: Vec::new(),
            agent_log: Vec::new(),
            session_progress: 0,
            session_status: "Idle".to_string(),
            index_status: IndexStatus::default(),
            plan_path: None,
            task_counts: HashMap::new(),
            chat_messages: Vec::new(),
            command_history: Vec::new(),
            history_index: 0,
        }
    }
}

pub struct App {
    pub state: AppState,
    pub events: Events,
    pub orchestrator: Option<Orchestrator>,
    pub session_store: Option<SessionStore>,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            state: AppState::default(),
            events: Events::new()?,
            orchestrator: None,
            session_store: None,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        // Load previous sessions from disk
        self.load_sessions()?;

        // Add welcome message for dashboard mode
        if self.state.session_status == "Dashboard" {
            self.state.chat_messages.push(ChatMessage {
                role: ChatRole::System,
                content: "Welcome to Telisq! Type a command or describe what you want to build.\nTry: 'help' for available commands, 'run' to execute a plan, 'plan' to create one.".to_string(),
                timestamp: Instant::now(),
            });
        }

        // Initialize terminal
        let mut terminal = start_tui()?;

        // Main event loop
        let result = self.main_loop(&mut terminal).await;

        // Cleanup terminal
        stop_tui(&mut terminal)?;

        result
    }

    async fn main_loop(&mut self, terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stderr>>) -> anyhow::Result<()> {
        loop {
            // Render the UI
            terminal.draw(|frame| self.render(frame))?;

            // Wait for next event
            let event = match self.events.next().await {
                Some(event) => event,
                None => continue,
            };

            if let Err(e) = self.handle_event(event).await {
                return Err(e);
            }
        }
    }

    fn load_sessions(&mut self) -> anyhow::Result<()> {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".telisq");
        let db_path = data_dir.join("telisq.db").to_string_lossy().to_string();

        let rt = tokio::runtime::Runtime::new()?;
        let store = rt.block_on(async {
            SessionStore::new(&db_path).await
        })?;

        let project_path = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let sessions = rt.block_on(async {
            store.list_sessions(&project_path).await
        })?;

        info!(session_count = sessions.len(), "Loaded sessions from store");

        self.session_store = Some(store);

        Ok(())
    }

    #[allow(dead_code)]
    fn save_session_snapshot(&mut self) -> anyhow::Result<()> {
        let store = match &self.session_store {
            Some(s) => s,
            None => {
                warn!("No session store available for snapshot");
                return Ok(());
            }
        };

        let session_id = match &self.state.session_id {
            Some(id) => SessionId::parse_str(id).map_err(|e| anyhow::anyhow!("Invalid session ID: {}", e))?,
            None => {
                warn!("No session ID available for snapshot");
                return Ok(());
            }
        };

        let rt = tokio::runtime::Runtime::new()?;
        
        rt.block_on(async {
            store.update_session_status(session_id, "running").await
        })?;

        info!("Session snapshot saved");
        Ok(())
    }

    async fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::Key(key) => self.handle_key_event(key).await,
            Event::Mouse(_) => Ok(()),     // Handle mouse events if needed
            Event::Resize(_, _) => Ok(()), // Handle resize events
            Event::Orchestrator(orch_event) => self.handle_orchestrator_event(orch_event).await,
            Event::Tick => Ok(()), // Periodic tick event for updates
        }
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> anyhow::Result<()> {
        match self.state.mode {
            AppMode::Normal => self.handle_normal_mode_key(key).await,
            AppMode::AskAgentInput => self.handle_ask_agent_mode_key(key).await,
            AppMode::CommandInput => self.handle_command_input_mode_key(key).await,
        }
    }

    /// Switches to AskAgentInput mode with the given question and options.
    pub fn switch_to_ask_mode(&mut self, question: String, options: Vec<String>) {
        self.state.mode = AppMode::AskAgentInput;
        self.state.ask_question = Some(question);
        self.state.ask_options = options;
        self.state.input_buffer.clear();
    }

    /// Switches back to Normal mode and returns the input buffer content.
    pub fn switch_to_normal_mode(&mut self) -> Option<String> {
        if self.state.mode == AppMode::AskAgentInput {
            self.state.mode = AppMode::Normal;
            let input = std::mem::take(&mut self.state.input_buffer);
            self.state.ask_question = None;
            self.state.ask_options.clear();
            Some(input)
        } else {
            None
        }
    }

    async fn handle_normal_mode_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<()> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('q') => {
                if self.orchestrator.is_some() && self.orchestrator.as_ref().unwrap().is_running() {
                    if self.state.quit_confirm {
                        return Err(anyhow::anyhow!("Quit"));
                    }
                    self.state.quit_confirm = true;
                } else {
                    return Err(anyhow::anyhow!("Quit"));
                }
            }
            KeyCode::Char('p') => self.state.active_panel = ActivePanel::Plan,
            KeyCode::Char('s') => self.state.active_panel = ActivePanel::Session,
            KeyCode::Char('a') => self.state.active_panel = ActivePanel::Agent,
            KeyCode::Char('i') => self.state.active_panel = ActivePanel::Index,
            KeyCode::Tab => {
                self.state.active_panel = match self.state.active_panel {
                    ActivePanel::Plan => ActivePanel::Session,
                    ActivePanel::Session => ActivePanel::Agent,
                    ActivePanel::Agent => ActivePanel::Index,
                    ActivePanel::Index => ActivePanel::Plan,
                };
            }
            KeyCode::Char(' ') => {
                if let Some(orch) = &self.orchestrator {
                    if orch.is_running() {
                        self.state
                            .agent_messages
                            .push("Pause/resume not yet implemented".to_string());
                    }
                }
            }
            // Enter command input mode
            KeyCode::Char(':') | KeyCode::Char('/') => {
                self.state.mode = AppMode::CommandInput;
                self.state.input_buffer.clear();
            }
            KeyCode::Enter => self.state.quit_confirm = false,
            KeyCode::Esc => self.state.quit_confirm = false,
            _ => {}
        }

        Ok(())
    }

    async fn handle_command_input_mode_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<()> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => {
                self.state.mode = AppMode::Normal;
                self.state.input_buffer.clear();
            }
            KeyCode::Enter => {
                let input = std::mem::take(&mut self.state.input_buffer);
                if !input.is_empty() {
                    // Save to command history
                    self.state.command_history.push(input.clone());
                    self.state.history_index = self.state.command_history.len();

                    // Add user message to chat
                    self.state.chat_messages.push(ChatMessage {
                        role: ChatRole::User,
                        content: input.clone(),
                        timestamp: Instant::now(),
                    });

                    // Process the command
                    self.process_command(&input).await;
                }
                self.state.mode = AppMode::Normal;
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            KeyCode::Delete => {
                if !self.state.input_buffer.is_empty() {
                    self.state.input_buffer.remove(self.state.input_buffer.len() - 1);
                }
            }
            KeyCode::Up => {
                if !self.state.command_history.is_empty() && self.state.history_index > 0 {
                    self.state.history_index -= 1;
                    self.state.input_buffer = self.state.command_history[self.state.history_index].clone();
                }
            }
            KeyCode::Down => {
                if self.state.history_index < self.state.command_history.len() {
                    self.state.history_index += 1;
                    if self.state.history_index < self.state.command_history.len() {
                        self.state.input_buffer = self.state.command_history[self.state.history_index].clone();
                    } else {
                        self.state.input_buffer.clear();
                    }
                }
            }
            KeyCode::Left | KeyCode::Right => {
                // TODO: Implement cursor movement
            }
            KeyCode::Char(c) => {
                self.state.input_buffer.push(c);
            }
            _ => {}
        }

        Ok(())
    }

    /// Process a command from the input buffer.
    async fn process_command(&mut self, command: &str) {
        let cmd = command.trim().to_lowercase();

        if cmd == "help" || cmd == "?" {
            self.add_agent_response(
                "Available commands:\n\
                 • help / ? — Show this help\n\
                 • run [plan] — Execute a plan (auto-discovers in plans/)\n\
                 • plan <goal> — Create a new plan\n\
                 • status — Show current plan progress\n\
                 • index build — Index codebase\n\
                 • index search <query> — Search indexed codebase\n\
                 • doctor — Run diagnostics\n\
                 • bootstrap — Create default config\n\
                 • clear — Clear chat history\n\
                 • quit / q — Exit Telisq"
            );
        } else if cmd == "quit" || cmd == "q" {
            // Will be handled by returning error
        } else if cmd.starts_with("run") {
            self.add_agent_response("Starting execution phase... (plan auto-discovery from plans/ directory)");
            // In a full implementation, this would spawn the orchestrator
            self.state.session_status = "Running".to_string();
        } else if cmd.starts_with("plan") {
            let goal = command.trim_start_matches("plan").trim();
            if goal.is_empty() {
                self.add_agent_response("Please provide a goal: plan <describe what you want to build>");
            } else {
                self.add_agent_response(&format!("Creating plan for: {}\n(Plan Agent will analyze your codebase and generate a plan)", goal));
            }
        } else if cmd.starts_with("status") {
            if self.state.tasks.is_empty() {
                self.add_agent_response("No active plan loaded. Use 'run' to start execution or 'plan <goal>' to create one.");
            } else {
                let completed = self.state.tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();
                let total = self.state.tasks.len();
                self.add_agent_response(&format!("Plan progress: {}/{} tasks completed ({}%)", completed, total, self.state.session_progress));
            }
        } else if cmd.starts_with("index") {
            self.add_agent_response("Index commands:\n• index build — Crawl and index the codebase\n• index search <query> — Semantic search\n• index status — Show index health");
        } else if cmd.starts_with("doctor") {
            self.add_agent_response("Running diagnostics... (use 'telisq doctor' from terminal for full check)");
        } else if cmd.starts_with("bootstrap") {
            self.add_agent_response("Bootstrapping configuration... (use 'telisq bootstrap' from terminal)");
        } else if cmd == "clear" {
            self.state.chat_messages.clear();
            self.add_agent_response("Chat history cleared.");
        } else {
            // Treat as natural language input
            self.add_agent_response(&format!("Received: \"{}\"\nIn a full implementation, this would be sent to the Orchestrator for processing. Use 'help' for available commands.", command));
        }
    }

    /// Add an agent response to the chat.
    fn add_agent_response(&mut self, message: &str) {
        self.state.chat_messages.push(ChatMessage {
            role: ChatRole::Agent,
            content: message.to_string(),
            timestamp: Instant::now(),
        });
    }

    async fn handle_ask_agent_mode_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<()> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => {
                self.state.input_buffer.clear();
                self.state.mode = AppMode::Normal;
                self.state.ask_question = None;
                self.state.ask_options.clear();
            }
            KeyCode::Enter => {
                self.state.mode = AppMode::Normal;
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            KeyCode::Delete => {
                if !self.state.input_buffer.is_empty() {
                    self.state.input_buffer.remove(self.state.input_buffer.len() - 1);
                }
            }
            KeyCode::Left | KeyCode::Right => {}
            KeyCode::Char(c) => {
                self.state.input_buffer.push(c);
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_orchestrator_event(&mut self, event: OrchestratorEvent) -> anyhow::Result<()> {
        use std::time::Instant;

        match event {
            OrchestratorEvent::StepStarted(task_id) => {
                self.state.current_step = Some(task_id.clone());
                self.state.session_status = "Running".to_string();

                if let Some(task) = self.state.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = TaskStatus::InProgress;
                }

                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message: format!("[~] Task {} started", task_id),
                });

                if self.state.agent_log.len() > 50 {
                    self.state.agent_log.drain(..self.state.agent_log.len() - 50);
                }
            }
            OrchestratorEvent::StepCompleted(task_id) => {
                self.state.agent_messages.push(format!("Completed step: {}", task_id));

                if let Some(task) = self.state.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = TaskStatus::Completed;
                }

                self.update_task_counts();

                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message: format!("[x] Task {} completed", task_id),
                });

                if self.state.agent_log.len() > 50 {
                    self.state.agent_log.drain(..self.state.agent_log.len() - 50);
                }

                self.update_session_progress();
            }
            OrchestratorEvent::StepFailed(task_id, error) => {
                self.state.agent_messages.push(format!("Failed step: {} - {}", task_id, error));

                if let Some(task) = self.state.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = TaskStatus::Failed;
                }

                self.update_task_counts();

                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message: format!("[!] Task {} failed: {}", task_id, error),
                });

                if self.state.agent_log.len() > 50 {
                    self.state.agent_log.drain(..self.state.agent_log.len() - 50);
                }

                self.update_session_progress();
            }
            OrchestratorEvent::AgentMessage(message) => {
                self.state.agent_messages.push(message.clone());

                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message,
                });

                if self.state.agent_log.len() > 50 {
                    self.state.agent_log.drain(..self.state.agent_log.len() - 50);
                }
            }
            OrchestratorEvent::PlanCompleted => {
                self.state.agent_messages.push("Plan completed".to_string());
                self.state.session_status = "Completed".to_string();
                self.state.session_progress = 100;
            }
            OrchestratorEvent::PlanMarkerUpdated(task_id, status) => {
                let marker = match status {
                    TaskStatus::Pending => " ",
                    TaskStatus::InProgress => "~",
                    TaskStatus::Completed => "x",
                    TaskStatus::Failed => "!",
                    TaskStatus::Skipped => "-",
                };

                if let Some(task) = self.state.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = status.clone();
                }

                self.update_task_counts();

                self.state.agent_messages.push(format!("Task {} marker updated to [{}]", task_id, marker));
            }
            OrchestratorEvent::SessionStopped(session_id) => {
                self.state.agent_messages.push(format!("Session {} stopped", session_id));
                self.state.session_status = "Stopped".to_string();
            }
            OrchestratorEvent::TaskRetry(task_id, attempt, error) => {
                self.state.agent_messages.push(format!(
                    "Task {} retry attempt {}: {}",
                    task_id, attempt, error
                ));

                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message: format!("Task {} retry {}: {}", task_id, attempt, error),
                });
            }
        }

        Ok(())
    }

    fn update_task_counts(&mut self) {
        self.state.task_counts.clear();
        for task in &self.state.tasks {
            *self.state.task_counts.entry(task.status).or_insert(0) += 1;
        }
    }

    fn update_session_progress(&mut self) {
        if self.state.tasks.is_empty() {
            self.state.session_progress = 0;
            return;
        }

        let completed = self
            .state
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count();

        self.state.session_progress =
            ((completed as u16 * 100) / self.state.tasks.len() as u16).min(100);
    }

    pub fn update_tasks(&mut self, tasks: Vec<TaskSpec>) {
        self.state.tasks = tasks
            .into_iter()
            .map(|spec| TaskDisplay {
                id: spec.id,
                title: spec.title,
                status: spec.status,
            })
            .collect();
        self.update_task_counts();
        self.update_session_progress();
    }

    pub fn render(&mut self, frame: &mut Frame) {
        match self.state.mode {
            AppMode::Normal => self.render_normal_mode(frame),
            AppMode::AskAgentInput => self.render_ask_agent_mode(frame),
            AppMode::CommandInput => self.render_command_input_mode(frame),
        }
    }

    fn render_normal_mode(&mut self, frame: &mut Frame) {
        // Main layout: sidebar, main content, index bar
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(55),
                Constraint::Percentage(25),
            ])
            .split(frame.size());

        components::sidebar::render(frame, main_layout[0], &self.state);

        let main_content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .split(main_layout[1]);

        match self.state.active_panel {
            ActivePanel::Plan => {
                components::plan_view::render(frame, main_content[0], &self.state);
                components::agent_panel::render(frame, main_content[1], &self.state);
            }
            ActivePanel::Session => {
                components::session_view::render(frame, main_content[0], &self.state);
                components::agent_panel::render(frame, main_content[1], &self.state);
            }
            ActivePanel::Agent => {
                components::plan_view::render(frame, main_content[0], &self.state);
                components::agent_panel::render(frame, main_content[1], &self.state);
            }
            ActivePanel::Index => {
                components::plan_view::render(frame, main_content[0], &self.state);
                components::agent_panel::render(frame, main_content[1], &self.state);
            }
        }

        components::index_bar::render(frame, main_layout[2], &self.state);

        // Command input hint bar (above status bar)
        self.render_command_hint(frame);

        // Status bar
        self.render_status_bar(frame);
    }

    fn render_command_input_mode(&mut self, frame: &mut Frame) {
        // Render the normal panels first
        self.render_normal_mode(frame);

        // Overlay input bar at bottom (above status bar)
        let area = frame.size();
        let input_height = 3u16;
        let input_y = area.height.saturating_sub(input_height + 1);

        let input_area = ratatui::layout::Rect::new(0, input_y, area.width, input_height);

        let input_text = format!("> {}", self.state.input_buffer);
        let input_block = Paragraph::new(input_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Command (Enter to submit, Esc to cancel)"),
            )
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(input_block, input_area);
    }

    fn render_command_hint(&self, frame: &mut Frame) {
        let area = frame.size();
        let hint_y = area.height.saturating_sub(2);
        let hint_area = ratatui::layout::Rect::new(0, hint_y, area.width, 1);

        let hint_text = " Press ':' or '/' to type a command | Tab: switch panel | q: quit | ↑↓: navigate";
        let hint = Paragraph::new(hint_text)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, hint_area);
    }

    fn render_ask_agent_mode(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Min(3),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(frame.size());

        let question_text = self.state.ask_question.as_deref().unwrap_or("No question");
        let question_block = Paragraph::new(question_text)
            .block(Block::default().borders(Borders::ALL).title("Question"))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));
        frame.render_widget(question_block, chunks[0]);

        let options_text = if self.state.ask_options.is_empty() {
            "No options available - free text input".to_string()
        } else {
            self.state.ask_options.join("\n")
        };
        let options_block = Paragraph::new(options_text)
            .block(Block::default().borders(Borders::ALL).title("Options"))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(options_block, chunks[1]);

        let input_text = format!("> {}", self.state.input_buffer);
        let input_block = Paragraph::new(input_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Input (Enter to submit, Esc to cancel)"),
            )
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(input_block, chunks[2]);

        self.render_status_bar(frame);
    }

    fn render_status_bar(&self, frame: &mut Frame) {
        let running = self.orchestrator.as_ref().is_some_and(|o| o.is_running());

        let status_text = match self.state.mode {
            AppMode::Normal | AppMode::CommandInput => {
                let running_status = if running { "▶ Running" } else { "⏸ Idle" };
                format!(
                    " Telisq v0.1.0 | {} | Session: {} | Progress: {}% | Panel: {:?} | Keys: q=quit, Tab=panel, p/s/a/i=switch, Space=pause",
                    running_status,
                    self.state.session_status,
                    self.state.session_progress,
                    self.state.active_panel
                )
            }
            AppMode::AskAgentInput => {
                " Telisq v0.1.0 | Mode: Ask Agent Input | Enter=submit, Esc=cancel".to_string()
            }
        };

        let block = Block::default()
            .borders(Borders::TOP)
            .title(" Status")
            .style(Style::default().fg(Color::Yellow));

        frame.render_widget(
            Paragraph::new(status_text).block(block),
            ratatui::layout::Rect::new(0, frame.size().height - 1, frame.size().width, 1),
        );
    }
}
