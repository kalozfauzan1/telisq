use crate::tui::components;
use crate::tui::events::{Event, Events};
use crate::tui::{start_tui, stop_tui};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;
use ratatui::Terminal;
use shared::types::{TaskId, TaskSpec, TaskStatus};
use std::collections::HashMap;
use std::time::Instant;
use telisq_core::orchestrator::{Orchestrator, OrchestratorEvent};

/// Application mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Normal operation mode.
    Normal,
    /// Asking for user input mode.
    AskAgentInput,
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
        }
    }
}

pub struct App {
    pub state: AppState,
    pub events: Events,
    pub orchestrator: Option<Orchestrator>,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            state: AppState::default(),
            events: Events::new()?,
            orchestrator: None,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        // Load previous sessions from disk
        self.load_sessions()?;

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
        // TODO: Implement loading sessions from disk
        Ok(())
    }

    #[allow(dead_code)]
    fn save_session_snapshot(&mut self) -> anyhow::Result<()> {
        // TODO: Implement saving session snapshot to disk
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
                        // Second 'q' press confirms quit
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
                // Cycle through panels
                self.state.active_panel = match self.state.active_panel {
                    ActivePanel::Plan => ActivePanel::Session,
                    ActivePanel::Session => ActivePanel::Agent,
                    ActivePanel::Agent => ActivePanel::Index,
                    ActivePanel::Index => ActivePanel::Plan,
                };
            }
            KeyCode::Char(' ') => {
                // Space bar to pause/resume (if orchestrator is running)
                if let Some(orch) = &self.orchestrator {
                    if orch.is_running() {
                        // TODO: Implement pause/resume
                        self.state
                            .agent_messages
                            .push("Pause/resume not yet implemented".to_string());
                    }
                }
            }
            KeyCode::Enter => self.state.quit_confirm = false,
            KeyCode::Esc => self.state.quit_confirm = false,
            _ => {}
        }

        Ok(())
    }

    async fn handle_ask_agent_mode_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<()> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => {
                // Cancel input and return to normal mode
                self.state.input_buffer.clear();
                self.state.mode = AppMode::Normal;
                self.state.ask_question = None;
                self.state.ask_options.clear();
            }
            KeyCode::Enter => {
                // Submit input and return to normal mode
                self.state.mode = AppMode::Normal;
                // The input buffer content will be retrieved by switch_to_normal_mode
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            KeyCode::Delete => {
                if !self.state.input_buffer.is_empty() {
                    self.state
                        .input_buffer
                        .remove(self.state.input_buffer.len() - 1);
                }
            }
            KeyCode::Left => {
                // TODO: Implement cursor movement
            }
            KeyCode::Right => {
                // TODO: Implement cursor movement
            }
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

                // Update task status to InProgress
                if let Some(task) = self.state.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = TaskStatus::InProgress;
                }

                // Add agent log entry
                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message: format!("[~] Task {} started", task_id),
                });

                // Keep only last 50 log entries
                if self.state.agent_log.len() > 50 {
                    self.state
                        .agent_log
                        .drain(..self.state.agent_log.len() - 50);
                }
            }
            OrchestratorEvent::StepCompleted(task_id) => {
                self.state
                    .agent_messages
                    .push(format!("Completed step: {}", task_id));

                // Update task status to Completed
                if let Some(task) = self.state.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = TaskStatus::Completed;
                }

                // Update task counts
                self.update_task_counts();

                // Add agent log entry
                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message: format!("[x] Task {} completed", task_id),
                });

                // Keep only last 50 log entries
                if self.state.agent_log.len() > 50 {
                    self.state
                        .agent_log
                        .drain(..self.state.agent_log.len() - 50);
                }

                // Update progress
                self.update_session_progress();
            }
            OrchestratorEvent::StepFailed(task_id, error) => {
                self.state
                    .agent_messages
                    .push(format!("Failed step: {} - {}", task_id, error));

                // Update task status to Failed
                if let Some(task) = self.state.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = TaskStatus::Failed;
                }

                // Update task counts
                self.update_task_counts();

                // Add agent log entry
                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message: format!("[!] Task {} failed: {}", task_id, error),
                });

                // Keep only last 50 log entries
                if self.state.agent_log.len() > 50 {
                    self.state
                        .agent_log
                        .drain(..self.state.agent_log.len() - 50);
                }

                // Update progress
                self.update_session_progress();
            }
            OrchestratorEvent::AgentMessage(message) => {
                self.state.agent_messages.push(message.clone());

                // Add agent log entry
                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message,
                });

                // Keep only last 50 log entries
                if self.state.agent_log.len() > 50 {
                    self.state
                        .agent_log
                        .drain(..self.state.agent_log.len() - 50);
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

                // Update task status
                if let Some(task) = self.state.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = status.clone();
                }

                // Update task counts
                self.update_task_counts();

                self.state
                    .agent_messages
                    .push(format!("Task {} marker updated to [{}]", task_id, marker));
            }
            OrchestratorEvent::SessionStopped(session_id) => {
                self.state
                    .agent_messages
                    .push(format!("Session {} stopped", session_id));
                self.state.session_status = "Stopped".to_string();
            }
            OrchestratorEvent::TaskRetry(task_id, attempt, error) => {
                self.state.agent_messages.push(format!(
                    "Task {} retry attempt {}: {}",
                    task_id, attempt, error
                ));

                // Add agent log entry
                self.state.agent_log.push(AgentLogEntry {
                    timestamp: Instant::now(),
                    message: format!("Task {} retry {}: {}", task_id, attempt, error),
                });
            }
        }

        Ok(())
    }

    /// Updates the task counts based on current task statuses.
    fn update_task_counts(&mut self) {
        self.state.task_counts.clear();
        for task in &self.state.tasks {
            *self.state.task_counts.entry(task.status).or_insert(0) += 1;
        }
    }

    /// Updates the session progress percentage based on completed tasks.
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

    /// Updates the task list from task specs.
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
        }
    }

    fn render_normal_mode(&mut self, frame: &mut Frame) {
        // Main layout with 3 columns: sidebar, main content, index bar
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20), // Sidebar (session info)
                Constraint::Percentage(55), // Main content
                Constraint::Percentage(25), // Index bar
            ])
            .split(frame.size());

        // Sidebar - session info
        components::sidebar::render(frame, main_layout[0], &self.state);

        // Main content - split vertically based on active panel
        let main_content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60), // Top panel
                Constraint::Percentage(40), // Bottom panel
            ])
            .split(main_layout[1]);

        // Render based on active panel
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

        // Index bar on the right
        components::index_bar::render(frame, main_layout[2], &self.state);

        // Status bar
        self.render_status_bar(frame);
    }

    fn render_ask_agent_mode(&mut self, frame: &mut Frame) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

        // Full screen layout for ask mode
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Question area
                Constraint::Min(3),    // Options area
                Constraint::Length(3), // Input area
                Constraint::Length(1), // Status bar
            ])
            .split(frame.size());

        // Question block
        let question_text = self.state.ask_question.as_deref().unwrap_or("No question");
        let question_block = Paragraph::new(question_text)
            .block(Block::default().borders(Borders::ALL).title("Question"))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));
        frame.render_widget(question_block, chunks[0]);

        // Options block
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

        // Input block
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

        // Status bar
        self.render_status_bar(frame);
    }

    fn render_status_bar(&self, frame: &mut Frame) {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let running = self.orchestrator.as_ref().is_some_and(|o| o.is_running());

        let status_text = match self.state.mode {
            AppMode::Normal => {
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
