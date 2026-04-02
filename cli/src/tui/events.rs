use crossterm::event::{self, Event as CEvent, KeyEvent};
use std::time::Duration;
use telisq_core::orchestrator::OrchestratorEvent;

/// Events that can occur in the TUI application.
#[derive(Debug)]
pub enum Event {
    /// Keyboard input event.
    Key(KeyEvent),
    /// Mouse event.
    Mouse(event::MouseEvent),
    /// Terminal resize event.
    Resize(u16, u16),
    /// Orchestrator event from the backend.
    Orchestrator(OrchestratorEvent),
    /// Tick event for periodic updates.
    Tick,
}

/// Event handler for the TUI application.
/// Manages both terminal events and orchestrator events.
pub struct Events {
    /// Orchestrator event receiver.
    orchestrator_rx: Option<tokio::sync::mpsc::Receiver<OrchestratorEvent>>,
}

impl Events {
    /// Creates a new Events instance.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            orchestrator_rx: None,
        })
    }

    /// Sets the orchestrator event receiver.
    pub fn set_orchestrator_rx(&mut self, rx: tokio::sync::mpsc::Receiver<OrchestratorEvent>) {
        self.orchestrator_rx = Some(rx);
    }

    /// Gets the next available event from either terminal or orchestrator.
    /// Returns None only on timeout (caller should continue polling).
    pub async fn next(&mut self) -> Option<Event> {
        // Check for terminal events first (non-blocking)
        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            match event::read().ok() {
                Some(CEvent::Key(key)) => return Some(Event::Key(key)),
                Some(CEvent::Mouse(mouse)) => return Some(Event::Mouse(mouse)),
                Some(CEvent::Resize(w, h)) => return Some(Event::Resize(w, h)),
                _ => {}
            }
        }

        // Check for orchestrator events
        if let Some(rx) = &mut self.orchestrator_rx {
            match rx.try_recv() {
                Ok(event) => return Some(Event::Orchestrator(event)),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    // Orchestrator channel closed, but we keep running
                }
            }
        }

        // Small sleep to prevent busy loop
        tokio::time::sleep(Duration::from_millis(50)).await;
        None
    }
}
