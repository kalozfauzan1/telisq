//! Index bar component for displaying index status and search functionality.

use crate::tui::app::AppState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

/// Renders the index bar component.
///
/// Displays:
/// - Index connection status (Ollama and Qdrant)
/// - Indexed file count and last update time
/// - Search input and results (when search is active)
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let index = &state.index_status;

    // Main block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Index")
        .style(Style::default().fg(Color::Cyan));

    // Build content lines
    let mut lines: Vec<String> = Vec::new();

    // Connection status section
    lines.push("── Connection Status ──".to_string());

    // Ollama status
    let ollama_status = if index.ollama_connected {
        "● Ollama: Connected"
    } else {
        "○ Ollama: Disconnected"
    };
    lines.push(ollama_status.to_string());

    // Qdrant status
    let qdrant_status = if index.qdrant_connected {
        "● Qdrant: Connected"
    } else {
        "○ Qdrant: Disconnected"
    };
    lines.push(qdrant_status.to_string());

    lines.push(String::new()); // Empty line

    // Index info section
    lines.push("── Index Info ──".to_string());
    lines.push(format!("Files indexed: {}", index.indexed_file_count));

    if let Some(last_update) = &index.last_update {
        lines.push(format!("Last update: {}", last_update));
    } else {
        lines.push("Last update: Never".to_string());
    }

    let indexing_status = if index.is_indexing {
        "Status: Indexing..."
    } else {
        "Status: Idle"
    };
    lines.push(indexing_status.to_string());

    lines.push(String::new()); // Empty line

    // Search section
    lines.push("── Search ──".to_string());

    if index.show_search {
        lines.push(format!("Query: {}", index.search_query));
        lines.push(String::new());

        if index.search_results.is_empty() {
            lines.push("No results found".to_string());
        } else {
            lines.push(format!("Results ({}):", index.search_results.len()));
            for (i, result) in index.search_results.iter().take(5).enumerate() {
                let preview = if result.content_preview.len() > 40 {
                    format!("{}...", &result.content_preview[..40])
                } else {
                    result.content_preview.clone()
                };
                lines.push(format!(
                    "  {}. [{:.2}] {} - {}",
                    i + 1,
                    result.score,
                    result.file_path,
                    preview
                ));
            }
        }
    } else {
        lines.push("Press '/' to search".to_string());
    }

    let content = lines.join("\n");
    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// Renders a simplified index status indicator.
/// Can be used in headers or compact views.
pub fn render_status_indicator(frame: &mut Frame, area: Rect, state: &AppState) {
    let index = &state.index_status;

    let (text, color) = if index.ollama_connected && index.qdrant_connected {
        ("● Index: Ready", Color::Green)
    } else if index.is_indexing {
        ("● Index: Indexing...", Color::Yellow)
    } else {
        ("○ Index: Not Ready", Color::Red)
    };

    let block = Block::default().borders(Borders::ALL).title("Index Status");

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(color));

    frame.render_widget(paragraph, area);
}
