// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use crate::tui::app::{AppState, ChatRole};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

/// Renders the agent panel component showing agent activity messages.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    // In dashboard mode, show chat history instead of agent activity
    if state.session_status == "Dashboard" && !state.chat_messages.is_empty() {
        render_chat_history(frame, area, state);
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Agent Activity");

    // Show last 15 agent messages
    let messages: Vec<ListItem> = state
        .agent_messages
        .iter()
        .rev()
        .take(15)
        .map(|msg| {
            let style = if msg.contains("Failed") || msg.contains("failed") || msg.contains("error")
            {
                Style::default().fg(Color::Red)
            } else if msg.contains("Completed")
                || msg.contains("completed")
                || msg.contains("success")
            {
                Style::default().fg(Color::Green)
            } else if msg.contains("Starting") || msg.contains("started") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(msg.as_str()).style(style)
        })
        .collect();

    if messages.is_empty() {
        let empty_msg =
            ListItem::new("No agent activity yet").style(Style::default().fg(Color::DarkGray));
        let list = List::new(vec![empty_msg]).block(block);
        frame.render_widget(list, area);
    } else {
        let list = List::new(messages).block(block);
        frame.render_widget(list, area);
    }
}

/// Renders chat history for dashboard mode.
fn render_chat_history(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Chat");

    let messages: Vec<ListItem> = state
        .chat_messages
        .iter()
        .rev()
        .take(20)
        .flat_map(|msg| {
            let (prefix, color) = match msg.role {
                ChatRole::User => ("You", Color::Cyan),
                ChatRole::Agent => ("Telisq", Color::Green),
                ChatRole::System => ("System", Color::Yellow),
            };

            let header = ListItem::new(format!("{}:", prefix)).style(Style::default().fg(color).add_modifier(ratatui::style::Modifier::BOLD));
            let content_lines: Vec<ListItem> = msg.content
                .lines()
                .map(|line| ListItem::new(format!("  {}", line)).style(Style::default().fg(Color::White)))
                .collect();

            let mut items = vec![header];
            items.extend(content_lines);
            items.push(ListItem::new("")); // Spacer
            items
        })
        .collect();

    if messages.is_empty() {
        let empty_msg =
            ListItem::new("No messages yet. Type ':' or '/' to start.").style(Style::default().fg(Color::DarkGray));
        let list = List::new(vec![empty_msg]).block(block);
        frame.render_widget(list, area);
    } else {
        let list = List::new(messages).block(block);
        frame.render_widget(list, area);
    }
}
