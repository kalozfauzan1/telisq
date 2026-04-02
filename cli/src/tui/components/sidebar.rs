use crate::tui::app::AppState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use shared::types::TaskStatus;

/// Renders the sidebar component with session information.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut items: Vec<ListItem> = Vec::new();

    // Session ID
    let session_text = state
        .session_id
        .as_ref()
        .map(|id| format!("Session: {}...", &id.to_string()[..8]))
        .unwrap_or_else(|| "Session: None".to_string());
    items.push(ListItem::new(session_text));

    // Session status
    let status_color = match state.session_status.as_str() {
        "Running" => Color::Green,
        "Completed" => Color::Blue,
        "Stopped" => Color::Red,
        "Idle" => Color::Yellow,
        _ => Color::White,
    };
    items.push(
        ListItem::new(format!("Status: {}", state.session_status))
            .style(Style::default().fg(status_color)),
    );

    // Task summary
    items.push(ListItem::new(""));
    items.push(ListItem::new("Tasks:").style(Style::default().fg(Color::Yellow)));

    let completed = state.task_counts.get(&TaskStatus::Completed).unwrap_or(&0);
    let in_progress = state.task_counts.get(&TaskStatus::InProgress).unwrap_or(&0);
    let failed = state.task_counts.get(&TaskStatus::Failed).unwrap_or(&0);
    let skipped = state.task_counts.get(&TaskStatus::Skipped).unwrap_or(&0);
    let pending = state.task_counts.get(&TaskStatus::Pending).unwrap_or(&0);

    items.push(ListItem::new(format!("  [x] Done: {}", completed)));
    items.push(ListItem::new(format!("  [~] Active: {}", in_progress)));
    items.push(ListItem::new(format!("  [!] Failed: {}", failed)));
    items.push(ListItem::new(format!("  [-] Skipped: {}", skipped)));
    items.push(ListItem::new(format!("  [ ] Pending: {}", pending)));

    // Progress
    items.push(ListItem::new(""));
    items.push(ListItem::new(format!(
        "Progress: {}%",
        state.session_progress
    )));

    // Current step if any
    if let Some(step) = &state.current_step {
        items.push(ListItem::new(""));
        items.push(
            ListItem::new(format!("Active: {}", step)).style(Style::default().fg(Color::Yellow)),
        );
    }

    // Quit confirmation
    if state.quit_confirm {
        items.push(ListItem::new(""));
        items.push(ListItem::new("Press 'q' again to quit").style(Style::default().fg(Color::Red)));
    }

    let block = Block::default().borders(Borders::ALL).title(" Session");

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
