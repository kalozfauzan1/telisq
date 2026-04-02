//! Session view component for displaying session information and task progress.

use crate::tui::app::AppState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};
use ratatui::Frame;
use shared::types::TaskStatus;

/// Renders the session view component.
///
/// Displays:
/// - Session information (ID, status, plan path)
/// - Task list with markers
/// - Progress bar for overall completion
/// - Agent activity log
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    // Split area into sections
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(5), // Session info
            ratatui::layout::Constraint::Length(3), // Progress bar
            ratatui::layout::Constraint::Min(5),    // Task list
        ])
        .split(area);

    // Render session info
    render_session_info(frame, chunks[0], state);

    // Render progress bar
    render_progress_bar(frame, chunks[1], state);

    // Render task list
    render_task_list(frame, chunks[2], state);
}

/// Renders session information section.
fn render_session_info(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut info_lines: Vec<String> = Vec::new();

    // Session ID
    if let Some(session_id) = &state.session_id {
        info_lines.push(format!("Session: {}", session_id));
    } else {
        info_lines.push("Session: No active session".to_string());
    }

    // Status
    info_lines.push(format!("Status: {}", state.session_status));

    // Plan path
    if let Some(plan_path) = &state.plan_path {
        info_lines.push(format!("Plan: {}", plan_path));
    } else {
        info_lines.push("Plan: Not loaded".to_string());
    }

    let content = info_lines.join("\n");
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Session Info");

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the progress bar.
fn render_progress_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let progress = state.session_progress as f64 / 100.0;

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Progress"))
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(progress)
        .label(format!(
            "{}/{} tasks ({}%)",
            state.task_counts.get(&TaskStatus::Completed).unwrap_or(&0),
            state.tasks.len(),
            state.session_progress
        ));

    frame.render_widget(gauge, area);
}

/// Renders the task list with markers.
fn render_task_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title(" Tasks");

    if state.tasks.is_empty() {
        let paragraph = Paragraph::new("No tasks loaded")
            .block(block.clone())
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    // Build task items with markers
    let items: Vec<ListItem> = state
        .tasks
        .iter()
        .map(|task| {
            let marker = match task.status {
                TaskStatus::Pending => " ",
                TaskStatus::InProgress => "~",
                TaskStatus::Completed => "x",
                TaskStatus::Failed => "!",
                TaskStatus::Skipped => "-",
            };

            let (color, style) = match task.status {
                TaskStatus::Completed => (Color::Green, Modifier::empty()),
                TaskStatus::InProgress => (Color::Yellow, Modifier::BOLD),
                TaskStatus::Failed => (Color::Red, Modifier::empty()),
                TaskStatus::Skipped => (Color::DarkGray, Modifier::empty()),
                TaskStatus::Pending => (Color::White, Modifier::empty()),
            };

            let text = format!("[{}] {} - {}", marker, task.id, task.title);
            ListItem::new(text).style(Style::default().fg(color).add_modifier(style))
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Renders the agent activity log section.
pub fn render_agent_log(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Agent Activity");

    if state.agent_log.is_empty() {
        let paragraph = Paragraph::new("No activity yet")
            .block(block.clone())
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    // Show last 20 log entries
    let entries: Vec<String> = state
        .agent_log
        .iter()
        .rev()
        .take(20)
        .map(|entry| entry.message.clone())
        .collect();

    let content = entries.join("\n");
    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// Renders a compact task summary.
pub fn render_task_summary(frame: &mut Frame, area: Rect, state: &AppState) {
    let completed = state.task_counts.get(&TaskStatus::Completed).unwrap_or(&0);
    let in_progress = state.task_counts.get(&TaskStatus::InProgress).unwrap_or(&0);
    let failed = state.task_counts.get(&TaskStatus::Failed).unwrap_or(&0);
    let skipped = state.task_counts.get(&TaskStatus::Skipped).unwrap_or(&0);
    let pending = state.task_counts.get(&TaskStatus::Pending).unwrap_or(&0);

    let summary = format!(
        "Tasks: [x]{} [~]{} [!]{} [-]{} [ ]{} | Total: {}",
        completed,
        in_progress,
        failed,
        skipped,
        pending,
        state.tasks.len()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Task Summary");

    let paragraph = Paragraph::new(summary).block(block);
    frame.render_widget(paragraph, area);
}
