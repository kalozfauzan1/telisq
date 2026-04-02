use crate::tui::app::AppState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;
use shared::types::TaskStatus;

/// Renders the plan view component showing tasks with their current markers.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title(" Plan");

    if state.plan_nodes.is_empty() {
        let paragraph = ratatui::widgets::Paragraph::new("No plan loaded")
            .block(block.clone())
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = state
        .plan_nodes
        .iter()
        .map(|node| {
            let (icon, color, style) = if state.current_step.as_ref() == Some(node) {
                ("⏳", Color::Yellow, Modifier::BOLD)
            } else {
                ("🔲", Color::White, Modifier::empty())
            };

            ListItem::new(format!("{} {}", icon, node))
                .style(Style::default().fg(color).add_modifier(style))
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
