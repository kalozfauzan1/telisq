// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use crate::tui::app::{AppState, ChatRole};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use shared::types::TaskStatus;

/// Renders the sidebar component with session information.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut items: Vec<ListItem> = Vec::new();

    // Title based on mode
    let title = if state.session_status == "Dashboard" {
        " Dashboard"
    } else {
        " Session"
    };

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
        "Dashboard" => Color::Cyan,
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

    // Plan path if available
    if let Some(plan) = &state.plan_path {
        items.push(ListItem::new(""));
        items.push(
            ListItem::new(format!("Plan: {}", plan))
                .style(Style::default().fg(Color::DarkGray)),
        );
    }

    // Quit confirmation
    if state.quit_confirm {
        items.push(ListItem::new(""));
        items.push(ListItem::new("Press 'q' again to quit").style(Style::default().fg(Color::Red)));
    }

    // Dashboard hint
    if state.session_status == "Dashboard" {
        items.push(ListItem::new(""));
        items.push(ListItem::new("Type ':' or '/' to start").style(Style::default().fg(Color::DarkGray)));
        items.push(ListItem::new("Type 'help' for commands").style(Style::default().fg(Color::DarkGray)));
    }

    let block = Block::default().borders(Borders::ALL).title(title);

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
