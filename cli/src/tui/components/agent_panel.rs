use crate::tui::app::AppState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

/// Renders the agent panel component showing agent activity messages.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
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
            // Color-code messages based on content
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
