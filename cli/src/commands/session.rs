use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use shared::config::AppConfig;
use shared::types::{SessionId, SessionState};
use std::path::PathBuf;
use telisq_core::session::store::SessionStore;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(about = "List and manage sessions")]
pub struct Session {
    #[command(subcommand)]
    command: SessionCommand,
}

#[derive(Subcommand)]
pub enum SessionCommand {
    /// List all sessions
    List,
    /// Resume a session by ID
    Resume {
        /// Session ID to resume
        #[arg(value_name = "ID")]
        id: String,

        /// Continue from a specific step
        #[arg(short, long)]
        continue_from: Option<String>,

        /// Dry run without making changes
        #[arg(short, long)]
        dry_run: bool,
    },
    /// Show details of a session
    Show {
        /// Session ID to show
        #[arg(value_name = "ID")]
        id: String,
    },
    /// Delete a session
    Delete {
        /// Session ID to delete
        #[arg(value_name = "ID")]
        id: String,
    },
    /// Export a session to JSON
    Export {
        /// Session ID to export
        #[arg(value_name = "ID")]
        id: String,

        /// Output file path (defaults to stdout)
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
}

impl Session {
    pub fn run(self) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        match &self.command {
            SessionCommand::List => self.list_sessions().await,
            SessionCommand::Resume {
                id,
                continue_from,
                dry_run,
            } => {
                self.resume_session(id.clone(), continue_from.clone(), *dry_run)
                    .await
            }
            SessionCommand::Show { id } => self.show_session(id.clone()).await,
            SessionCommand::Delete { id } => self.delete_session(id.clone()).await,
            SessionCommand::Export { id, output } => {
                self.export_session(id.clone(), output.clone()).await
            }
        }
    }

    /// Lists all sessions from SQLite
    async fn list_sessions(&self) -> anyhow::Result<()> {
        info!("Listing sessions");

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;

        // Initialize session store
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".telisq");
        let db_path = data_dir.join("telisq.db").to_string_lossy().to_string();
        let store = SessionStore::new(&db_path)
            .await
            .context("Failed to initialize session store")?;

        // List sessions - use current directory as project path filter
        let project_path = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let sessions = store
            .list_sessions(&project_path)
            .await
            .context("Failed to list sessions")?;

        if sessions.is_empty() {
            println!("No sessions found");
            return Ok(());
        }

        println!("📋 Sessions ({} total):", sessions.len());
        println!();

        for session in &sessions {
            let status_icon = match session.state {
                SessionState::Running => "🟢",
                SessionState::Paused => "🟡",
                SessionState::Completed => "✅",
                SessionState::Canceled => "❌",
            };

            println!("  {} {} ({})", status_icon, session.id, session.name);
            println!("     Plan: {}", session.plan_path.display());
            println!();
        }

        Ok(())
    }

    /// Resumes a session by ID
    async fn resume_session(
        &self,
        id: String,
        continue_from: Option<String>,
        dry_run: bool,
    ) -> anyhow::Result<()> {
        info!(session_id = %id, "Resuming session");

        // Parse session ID
        let session_id =
            SessionId::parse_str(&id).map_err(|e| anyhow!("Invalid session ID '{}': {}", id, e))?;

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;

        // Initialize session store
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".telisq");
        let db_path = data_dir.join("telisq.db").to_string_lossy().to_string();
        let store = SessionStore::new(&db_path)
            .await
            .context("Failed to initialize session store")?;

        // Load session
        let session = store
            .load_session(session_id.clone())
            .await
            .context("Failed to load session")?
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;

        println!("🔄 Resuming session: {}", id);
        println!("   Name: {}", session.name);
        println!("   Plan: {}", session.plan_path.display());
        println!("   Status: {:?}", session.state);
        if dry_run {
            println!("   Dry run: enabled");
        }
        if let Some(ref task_id) = continue_from {
            println!("   Continue from: {}", task_id);
        }

        // TODO: Wire to orchestrator for actual resume execution
        println!("\n⚠️  Session resume wired - orchestrator execution not yet fully connected");

        Ok(())
    }

    /// Shows details of a session
    async fn show_session(&self, id: String) -> anyhow::Result<()> {
        info!(session_id = %id, "Showing session");

        // Parse session ID
        let session_id =
            SessionId::parse_str(&id).map_err(|e| anyhow!("Invalid session ID '{}': {}", id, e))?;

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;

        // Initialize session store
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".telisq");
        let db_path = data_dir.join("telisq.db").to_string_lossy().to_string();
        let store = SessionStore::new(&db_path)
            .await
            .context("Failed to initialize session store")?;

        // Load session
        let session = store
            .load_session(session_id.clone())
            .await
            .context("Failed to load session")?
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;

        println!("📋 Session Details");
        println!("==================");
        println!("  ID: {}", session.id);
        println!("  Name: {}", session.name);
        println!("  Plan: {}", session.plan_path.display());
        println!("  Status: {:?}", session.state);

        // Load events
        let events = store
            .load_events(session_id.clone())
            .await
            .context("Failed to load session events")?;
        println!("  Events: {}", events.len());

        // Show recent events
        if !events.is_empty() {
            println!("\n📜 Recent Events (last 10):");
            for event in events.iter().rev().take(10) {
                println!("  - {:?}", event);
            }
        }

        Ok(())
    }

    /// Deletes a session
    async fn delete_session(&self, id: String) -> anyhow::Result<()> {
        info!(session_id = %id, "Deleting session");

        // Parse session ID
        let session_id =
            SessionId::parse_str(&id).map_err(|e| anyhow!("Invalid session ID '{}': {}", id, e))?;

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;

        // Initialize session store
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".telisq");
        let db_path = data_dir.join("telisq.db").to_string_lossy().to_string();
        let store = SessionStore::new(&db_path)
            .await
            .context("Failed to initialize session store")?;

        // Check session exists
        let session = store
            .load_session(session_id.clone())
            .await
            .context("Failed to load session")?
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;

        // TODO: Implement delete_session in SessionStore
        // For now, mark the session as canceled
        store
            .update_session_status(session_id.clone(), "canceled")
            .await
            .context("Failed to update session status")?;

        println!("✅ Session marked as canceled: {}", id);
        println!("   Name: {}", session.name);
        println!("   Plan: {}", session.plan_path.display());
        println!("   Note: Full deletion requires SessionStore.delete_session() implementation");

        Ok(())
    }

    /// Exports a session to JSON
    async fn export_session(&self, id: String, output: Option<PathBuf>) -> anyhow::Result<()> {
        info!(session_id = %id, "Exporting session");

        // Parse session ID
        let session_id =
            SessionId::parse_str(&id).map_err(|e| anyhow!("Invalid session ID '{}': {}", id, e))?;

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;

        // Initialize session store
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".telisq");
        let db_path = data_dir.join("telisq.db").to_string_lossy().to_string();
        let store = SessionStore::new(&db_path)
            .await
            .context("Failed to initialize session store")?;

        // Load session
        let session = store
            .load_session(session_id.clone())
            .await
            .context("Failed to load session")?
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;

        // Load events
        let events = store
            .load_events(session_id.clone())
            .await
            .context("Failed to load session events")?;

        // Create export data
        let export_data = SessionExport {
            session_id: id.clone(),
            name: session.name.clone(),
            plan_path: session.plan_path.to_string_lossy().to_string(),
            state: format!("{:?}", session.state),
            events: events.iter().map(|e| format!("{:?}", e)).collect(),
        };

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&export_data)
            .context("Failed to serialize session to JSON")?;

        // Output
        match output {
            Some(path) => {
                std::fs::write(&path, &json).context("Failed to write export file")?;
                println!("✅ Session exported to: {}", path.display());
            }
            None => {
                println!("{}", json);
            }
        }

        Ok(())
    }
}

/// Export data structure for session export
#[derive(Debug, Serialize, Deserialize)]
struct SessionExport {
    session_id: String,
    name: String,
    plan_path: String,
    state: String,
    events: Vec<String>,
}
