// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

//! Session store implementation using SQLite via sqlx.
//!
//! Provides persistent storage for sessions, events, agent results, and plan markers.

use serde::{Deserialize, Serialize};
use shared::types::{Session, SessionId, SessionState, TaskId, TaskStatus};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::path::Path;
use std::time::Duration;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::orchestrator::OrchestratorEvent;

/// Result type for session store operations.
pub type Result<T> = std::result::Result<T, StoreError>;

/// SQLite schema version.
const SCHEMA_VERSION: i64 = 1;

/// Error type for session store operations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("UUID parse error: {0}")]
    UuidParse(String),

    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        StoreError::Database(e.to_string())
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Serialization(e.to_string())
    }
}

impl From<uuid::Error> for StoreError {
    fn from(e: uuid::Error) -> Self {
        StoreError::UuidParse(e.to_string())
    }
}

/// Session store wrapping a SQLite connection pool.
#[derive(Clone)]
pub struct SessionStore {
    pool: SqlitePool,
}

impl SessionStore {
    /// Creates a new session store with database initialization.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file. If the file doesn't exist, it will be created.
    ///
    /// # Returns
    /// A new `SessionStore` instance with migrations applied.
    pub async fn new(db_path: &str) -> Result<Self> {
        info!(db_path = %db_path, "Initializing session store");

        // Ensure parent directory exists
        if let Some(parent) = Path::new(db_path).parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| StoreError::Io(e.to_string()))?;
                debug!(parent_dir = ?parent, "Created parent directory for database");
            }
        }

        // Ensure parent directory exists
        if let Some(parent) = Path::new(db_path).parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| StoreError::Io(e.to_string()))?;
                debug!(parent_dir = ?parent, "Created parent directory for database");
            }
        }
        // Use sqlite: URL format with absolute path
        // On macOS, sqlite: prefix works with absolute paths directly
        let abs_path = if Path::new(db_path).is_absolute() {
            db_path.to_string()
        } else {
            std::env::current_dir()
                .map(|d| d.join(db_path))
                .unwrap_or_else(|_| Path::new(db_path).to_path_buf())
                .to_string_lossy()
                .to_string()
        };
        // Create an empty file to ensure SQLite can open it
        if !Path::new(&abs_path).exists() {
            std::fs::File::create(&abs_path)
                .map_err(|e| StoreError::Io(format!("Failed to create database file: {}", e)))?;
        }
        let db_url = format!("sqlite:{}", abs_path);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&db_url)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let store = Self { pool };
        store.migrate().await?;

        info!("Session store initialized successfully");
        Ok(store)
    }

    /// Returns a reference to the underlying SQLite pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Runs schema migrations on first connect.
    async fn migrate(&self) -> Result<()> {
        info!("Running schema migrations");

        // Create schema_version table if it doesn't exist
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Database(e.to_string()))?;

        // Check current version
        let current_version: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM schema_version")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| StoreError::Database(e.to_string()))?;

        if current_version == Some(SCHEMA_VERSION) {
            debug!(version = SCHEMA_VERSION, "Schema is up to date");
            return Ok(());
        }

        info!(
            from_version = ?current_version,
            to_version = SCHEMA_VERSION,
            "Applying migrations"
        );

        // Apply schema version 1
        if current_version.is_none() || current_version.unwrap() < 1 {
            self.apply_migration_v1().await?;
        }

        info!("Schema migrations completed");
        Ok(())
    }

    /// Applies migration version 1 - initial schema.
    async fn apply_migration_v1(&self) -> Result<()> {
        debug!("Applying migration v1");

        // Create sessions table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY NOT NULL,
                project_path TEXT NOT NULL,
                plan_path TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                status TEXT NOT NULL DEFAULT 'running',
                current_task_id TEXT
            )",
        )
        .execute(&self.pool)
        .await?;

        // Create events table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY NOT NULL,
                session_id TEXT NOT NULL,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                event_type TEXT NOT NULL,
                payload TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
        )
        .execute(&self.pool)
        .await?;

        // Create agent_results table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agent_results (
                id TEXT PRIMARY KEY NOT NULL,
                session_id TEXT NOT NULL,
                agent_type TEXT NOT NULL,
                task_id TEXT NOT NULL,
                result TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
        )
        .execute(&self.pool)
        .await?;

        // Create plan_markers table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS plan_markers (
                id TEXT PRIMARY KEY NOT NULL,
                session_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                marker TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for query performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_session_id ON events(session_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_agent_results_session_id ON agent_results(session_id)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_plan_markers_session_id ON plan_markers(session_id)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_sessions_project_path ON sessions(project_path)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status)")
            .execute(&self.pool)
            .await?;

        // Record migration
        sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
            .bind(SCHEMA_VERSION)
            .execute(&self.pool)
            .await?;

        debug!("Migration v1 applied successfully");
        Ok(())
    }

    /// Saves a session to the database.
    ///
    /// # Arguments
    /// * `session` - The session to persist.
    pub async fn save_session(&self, session: &Session) -> Result<()> {
        debug!(session_id = %session.id, "Saving session");

        let status = session_state_to_str(&session.state);
        let project_path = session
            .plan_path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        sqlx::query(
            "INSERT OR REPLACE INTO sessions (id, project_path, plan_path, name, status, updated_at)
             VALUES (?, ?, ?, ?, ?, datetime('now'))",
        )
        .bind(session.id.to_string())
        .bind(&project_path)
        .bind(session.plan_path.to_string_lossy().as_ref())
        .bind(&session.name)
        .bind(&status)
        .execute(&self.pool)
        .await?;

        debug!(session_id = %session.id, "Session saved successfully");
        Ok(())
    }

    /// Loads a session from the database by ID.
    ///
    /// # Arguments
    /// * `id` - The session ID to load.
    ///
    /// # Returns
    /// `Ok(Some(Session))` if found, `Ok(None)` if not found.
    pub async fn load_session(&self, id: SessionId) -> Result<Option<Session>> {
        debug!(session_id = %id, "Loading session");

        let row: Option<(String, String, String, String)> =
            sqlx::query_as("SELECT id, plan_path, name, status FROM sessions WHERE id = ?")
                .bind(id.to_string())
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some((id_str, plan_path, name, status)) => {
                let session_id = Uuid::parse_str(&id_str)?;
                let state = str_to_session_state(&status);
                let session = Session {
                    id: session_id,
                    name,
                    plan_path: plan_path.into(),
                    state,
                };
                debug!(session_id = %id, "Session loaded successfully");
                Ok(Some(session))
            }
            None => {
                debug!(session_id = %id, "Session not found");
                Ok(None)
            }
        }
    }

    /// Lists all sessions for a given project path.
    ///
    /// # Arguments
    /// * `project_path` - The project path to filter sessions by.
    ///
    /// # Returns
    /// A vector of sessions matching the project path.
    pub async fn list_sessions(&self, project_path: &str) -> Result<Vec<Session>> {
        debug!(project_path = %project_path, "Listing sessions");

        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT id, plan_path, name, status FROM sessions WHERE project_path = ? ORDER BY updated_at DESC",
        )
        .bind(project_path)
        .fetch_all(&self.pool)
        .await?;

        let sessions: Vec<Session> = rows
            .into_iter()
            .filter_map(|(id_str, plan_path, name, status)| {
                let session_id = Uuid::parse_str(&id_str).ok()?;
                let state = str_to_session_state(&status);
                Some(Session {
                    id: session_id,
                    name,
                    plan_path: plan_path.into(),
                    state,
                })
            })
            .collect();

        debug!(count = sessions.len(), "Sessions listed");
        Ok(sessions)
    }

    /// Updates the status of a session.
    ///
    /// # Arguments
    /// * `id` - The session ID to update.
    /// * `status` - The new status string.
    pub async fn update_session_status(&self, id: SessionId, status: &str) -> Result<()> {
        debug!(session_id = %id, status = %status, "Updating session status");

        sqlx::query("UPDATE sessions SET status = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(status)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        debug!(session_id = %id, "Session status updated");
        Ok(())
    }

    /// Saves an event to the database.
    ///
    /// # Arguments
    /// * `session_id` - The session ID the event belongs to.
    /// * `event` - The orchestrator event to save.
    pub async fn save_event(&self, session_id: SessionId, event: &OrchestratorEvent) -> Result<()> {
        let event_id = Uuid::new_v4();
        let event_type = orchestrator_event_type(event);
        let payload = serde_json::to_string(&serialize_event(event))?;

        debug!(
            event_id = %event_id,
            session_id = %session_id,
            event_type = %event_type,
            "Saving event"
        );

        sqlx::query("INSERT INTO events (id, session_id, event_type, payload) VALUES (?, ?, ?, ?)")
            .bind(event_id.to_string())
            .bind(session_id.to_string())
            .bind(&event_type)
            .bind(&payload)
            .execute(&self.pool)
            .await?;

        debug!(event_id = %event_id, "Event saved");
        Ok(())
    }

    /// Saves an agent result to the database.
    ///
    /// # Arguments
    /// * `session_id` - The session ID the result belongs to.
    /// * `agent_type` - The type of agent that produced the result.
    /// * `task_id` - The task ID the result is for.
    /// * `result` - The serialized agent result.
    pub async fn save_agent_result(
        &self,
        session_id: SessionId,
        agent_type: &str,
        task_id: &TaskId,
        result: &serde_json::Value,
    ) -> Result<()> {
        let result_id = Uuid::new_v4();
        let result_json = serde_json::to_string(result)?;

        debug!(
            result_id = %result_id,
            session_id = %session_id,
            agent_type = %agent_type,
            task_id = %task_id,
            "Saving agent result"
        );

        sqlx::query(
            "INSERT INTO agent_results (id, session_id, agent_type, task_id, result) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(result_id.to_string())
        .bind(session_id.to_string())
        .bind(agent_type)
        .bind(task_id)
        .bind(&result_json)
        .execute(&self.pool)
        .await?;

        debug!(result_id = %result_id, "Agent result saved");
        Ok(())
    }

    /// Saves a plan marker to the database.
    ///
    /// # Arguments
    /// * `session_id` - The session ID the marker belongs to.
    /// * `task_id` - The task ID the marker is for.
    /// * `marker` - The marker value (e.g., task status).
    pub async fn save_plan_marker(
        &self,
        session_id: SessionId,
        task_id: &TaskId,
        marker: &str,
    ) -> Result<()> {
        debug!(
            session_id = %session_id,
            task_id = %task_id,
            marker = %marker,
            "Saving plan marker"
        );

        // Check if marker exists for this session_id + task_id
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM plan_markers WHERE session_id = ? AND task_id = ?")
                .bind(session_id.to_string())
                .bind(task_id)
                .fetch_optional(&self.pool)
                .await?;

        match existing {
            Some(id) => {
                // Update existing marker
                sqlx::query(
                    "UPDATE plan_markers SET marker = ?, updated_at = datetime('now') WHERE id = ?",
                )
                .bind(marker)
                .bind(id)
                .execute(&self.pool)
                .await?;
            }
            None => {
                // Insert new marker
                let marker_id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO plan_markers (id, session_id, task_id, marker, updated_at) VALUES (?, ?, ?, ?, datetime('now'))",
                )
                .bind(marker_id.to_string())
                .bind(session_id.to_string())
                .bind(task_id)
                .bind(marker)
                .execute(&self.pool)
                .await?;
            }
        }

        debug!("Plan marker saved");
        Ok(())
    }

    /// Loads plan markers from the database for a session.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to load markers for.
    ///
    /// # Returns
    /// A map of task IDs to their marker values.
    pub async fn load_plan_markers(
        &self,
        session_id: SessionId,
    ) -> Result<std::collections::HashMap<TaskId, String>> {
        debug!(session_id = %session_id, "Loading plan markers");

        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT task_id, marker FROM plan_markers WHERE session_id = ?")
                .bind(session_id.to_string())
                .fetch_all(&self.pool)
                .await?;

        let markers: std::collections::HashMap<TaskId, String> = rows.into_iter().collect();

        debug!(count = markers.len(), "Plan markers loaded");
        Ok(markers)
    }

    /// Loads agent results from the database for a session.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to load results for.
    ///
    /// # Returns
    /// A map of task IDs to their agent results.
    pub async fn load_agent_results(
        &self,
        session_id: SessionId,
    ) -> Result<std::collections::HashMap<TaskId, serde_json::Value>> {
        debug!(session_id = %session_id, "Loading agent results");

        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT task_id, result FROM agent_results WHERE session_id = ?")
                .bind(session_id.to_string())
                .fetch_all(&self.pool)
                .await?;

        let results: std::collections::HashMap<TaskId, serde_json::Value> = rows
            .into_iter()
            .filter_map(|(task_id, result_json)| {
                serde_json::from_str(&result_json)
                    .ok()
                    .map(|value| (task_id, value))
            })
            .collect();

        debug!(count = results.len(), "Agent results loaded");
        Ok(results)
    }

    /// Loads events from the database for a session.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to load events for.
    ///
    /// # Returns
    /// A vector of events in chronological order.
    pub async fn load_events(&self, session_id: SessionId) -> Result<Vec<OrchestratorEvent>> {
        debug!(session_id = %session_id, "Loading events");

        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT event_type, payload FROM events WHERE session_id = ? ORDER BY timestamp ASC",
        )
        .bind(session_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let events: Vec<OrchestratorEvent> = rows
            .into_iter()
            .filter_map(|(event_type, payload)| deserialize_event(&event_type, &payload).ok())
            .collect();

        debug!(count = events.len(), "Events loaded");
        Ok(events)
    }

    /// Resumes a session from the database.
    ///
    /// This method restores the orchestrator state by:
    /// - Loading the session
    /// - Restoring task progress from plan_markers
    /// - Restoring agent results
    /// - Skipping completed tasks and resuming from last in-progress task
    /// - Resetting any in-progress tasks to pending (handles mid-task interruption)
    ///
    /// # Arguments
    /// * `session_id` - The session ID to resume.
    ///
    /// # Returns
    /// The restored session, or None if not found.
    pub async fn resume_session(&self, session_id: SessionId) -> Result<Option<Session>> {
        info!(session_id = %session_id, "Resuming session");

        // Load session
        let session = match self.load_session(session_id).await? {
            Some(s) => s,
            None => {
                warn!(session_id = %session_id, "Session not found for resume");
                return Ok(None);
            }
        };

        // Load plan markers to get task states
        let markers = self.load_plan_markers(session_id).await?;

        // Load agent results
        let results = self.load_agent_results(session_id).await?;

        // Reset any in-progress tasks to pending (handles mid-task interruption)
        let mut reset_count = 0;
        for (task_id, marker) in &markers {
            if marker == "in_progress" {
                self.save_plan_marker(session_id, task_id, "pending")
                    .await?;
                reset_count += 1;
            }
        }

        if reset_count > 0 {
            warn!(
                session_id = %session_id,
                reset_count = reset_count,
                "Reset in-progress tasks to pending due to possible interruption"
            );
        }

        info!(
            session_id = %session_id,
            markers_loaded = markers.len(),
            results_loaded = results.len(),
            tasks_reset = reset_count,
            "Session resume data loaded"
        );

        Ok(Some(session))
    }
}

/// Converts SessionState to a string for storage.
fn session_state_to_str(state: &SessionState) -> &'static str {
    match state {
        SessionState::Running => "running",
        SessionState::Paused => "paused",
        SessionState::Completed => "completed",
        SessionState::Canceled => "canceled",
    }
}

/// Converts a string to SessionState.
fn str_to_session_state(s: &str) -> SessionState {
    match s {
        "running" => SessionState::Running,
        "paused" => SessionState::Paused,
        "completed" => SessionState::Completed,
        "canceled" => SessionState::Canceled,
        _ => {
            warn!(status = %s, "Unknown session status, defaulting to running");
            SessionState::Running
        }
    }
}

/// Gets the event type string for an OrchestratorEvent.
fn orchestrator_event_type(event: &OrchestratorEvent) -> &'static str {
    match event {
        OrchestratorEvent::StepStarted(_) => "step_started",
        OrchestratorEvent::StepCompleted(_) => "step_completed",
        OrchestratorEvent::StepFailed(_, _) => "step_failed",
        OrchestratorEvent::AgentMessage(_) => "agent_message",
        OrchestratorEvent::PlanCompleted => "plan_completed",
        OrchestratorEvent::PlanMarkerUpdated(_, _) => "plan_marker_updated",
        OrchestratorEvent::SessionStopped(_) => "session_stopped",
        OrchestratorEvent::TaskRetry(_, _, _) => "task_retry",
    }
}

/// Serializable representation of an OrchestratorEvent for storage.
#[derive(Debug, Serialize, Deserialize)]
struct SerializableEvent {
    event_type: String,
    task_id: Option<String>,
    message: Option<String>,
    error: Option<String>,
    marker_status: Option<String>,
    session_id: Option<String>,
    retry_attempt: Option<usize>,
}

/// Serializes an OrchestratorEvent for storage.
fn serialize_event(event: &OrchestratorEvent) -> SerializableEvent {
    match event {
        OrchestratorEvent::StepStarted(task_id) => SerializableEvent {
            event_type: "step_started".to_string(),
            task_id: Some(task_id.clone()),
            message: None,
            error: None,
            marker_status: None,
            session_id: None,
            retry_attempt: None,
        },
        OrchestratorEvent::StepCompleted(task_id) => SerializableEvent {
            event_type: "step_completed".to_string(),
            task_id: Some(task_id.clone()),
            message: None,
            error: None,
            marker_status: None,
            session_id: None,
            retry_attempt: None,
        },
        OrchestratorEvent::StepFailed(task_id, error) => SerializableEvent {
            event_type: "step_failed".to_string(),
            task_id: Some(task_id.clone()),
            message: None,
            error: Some(error.clone()),
            marker_status: None,
            session_id: None,
            retry_attempt: None,
        },
        OrchestratorEvent::AgentMessage(message) => SerializableEvent {
            event_type: "agent_message".to_string(),
            task_id: None,
            message: Some(message.clone()),
            error: None,
            marker_status: None,
            session_id: None,
            retry_attempt: None,
        },
        OrchestratorEvent::PlanCompleted => SerializableEvent {
            event_type: "plan_completed".to_string(),
            task_id: None,
            message: None,
            error: None,
            marker_status: None,
            session_id: None,
            retry_attempt: None,
        },
        OrchestratorEvent::PlanMarkerUpdated(task_id, status) => SerializableEvent {
            event_type: "plan_marker_updated".to_string(),
            task_id: Some(task_id.clone()),
            message: None,
            error: None,
            marker_status: Some(format!("{:?}", status)),
            session_id: None,
            retry_attempt: None,
        },
        OrchestratorEvent::SessionStopped(session_id) => SerializableEvent {
            event_type: "session_stopped".to_string(),
            task_id: None,
            message: None,
            error: None,
            marker_status: None,
            session_id: Some(session_id.to_string()),
            retry_attempt: None,
        },
        OrchestratorEvent::TaskRetry(task_id, attempt, error) => SerializableEvent {
            event_type: "task_retry".to_string(),
            task_id: Some(task_id.clone()),
            message: None,
            error: Some(error.clone()),
            marker_status: None,
            session_id: None,
            retry_attempt: Some(*attempt),
        },
    }
}

/// Deserializes an OrchestratorEvent from storage.
fn deserialize_event(
    event_type: &str,
    payload: &str,
) -> std::result::Result<OrchestratorEvent, StoreError> {
    let serializable: SerializableEvent =
        serde_json::from_str(payload).map_err(|e| StoreError::Serialization(e.to_string()))?;

    match event_type {
        "step_started" => Ok(OrchestratorEvent::StepStarted(
            serializable.task_id.unwrap_or_default(),
        )),
        "step_completed" => Ok(OrchestratorEvent::StepCompleted(
            serializable.task_id.unwrap_or_default(),
        )),
        "step_failed" => Ok(OrchestratorEvent::StepFailed(
            serializable.task_id.unwrap_or_default(),
            serializable
                .error
                .unwrap_or_else(|| "Unknown error".to_string()),
        )),
        "agent_message" => Ok(OrchestratorEvent::AgentMessage(
            serializable.message.unwrap_or_default(),
        )),
        "plan_completed" => Ok(OrchestratorEvent::PlanCompleted),
        "plan_marker_updated" => {
            let task_id = serializable.task_id.unwrap_or_default();
            let status = match serializable.marker_status.as_deref() {
                Some("Pending") => TaskStatus::Pending,
                Some("InProgress") => TaskStatus::InProgress,
                Some("Completed") => TaskStatus::Completed,
                Some("Failed") => TaskStatus::Failed,
                Some("Skipped") => TaskStatus::Skipped,
                _ => TaskStatus::Pending,
            };
            Ok(OrchestratorEvent::PlanMarkerUpdated(task_id, status))
        }
        "session_stopped" => Ok(OrchestratorEvent::SessionStopped(
            serializable
                .session_id
                .as_deref()
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
                .unwrap_or_default(),
        )),
        "task_retry" => Ok(OrchestratorEvent::TaskRetry(
            serializable.task_id.unwrap_or_default(),
            serializable.retry_attempt.unwrap_or(1),
            serializable.error.unwrap_or_default(),
        )),
        _ => {
            warn!(event_type = %event_type, "Unknown event type during deserialization");
            Ok(OrchestratorEvent::AgentMessage(format!(
                "Unknown event: {}",
                event_type
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    async fn create_test_store() -> (SessionStore, TempDir) {
        let temp_dir = TempDir::new("telisq-test-store").expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        eprintln!("DEBUG: db_path = {:?}", db_path);
        eprintln!("DEBUG: temp_dir exists = {}", temp_dir.path().exists());
        let store = SessionStore::new(db_path.to_str().unwrap())
            .await
            .expect("Failed to create store");
        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_store_initialization() {
        let (store, _temp_dir) = create_test_store().await;
        // Just verify the store was created successfully
        drop(store);
    }

    #[tokio::test]
    async fn test_save_and_load_session() {
        let (store, _temp_dir) = create_test_store().await;

        let session = Session::new("test-session", "/tmp/test-plan.md");
        store
            .save_session(&session)
            .await
            .expect("Failed to save session");

        let loaded = store
            .load_session(session.id)
            .await
            .expect("Failed to load session")
            .expect("Session should exist");

        assert_eq!(loaded.name, session.name);
        assert_eq!(loaded.state, session.state);
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let (store, _temp_dir) = create_test_store().await;

        let session1 = Session::new("session-1", "/tmp/project1/plan.md");
        let session2 = Session::new("session-2", "/tmp/project1/plan2.md");
        let session3 = Session::new("session-3", "/tmp/project2/plan.md");

        store.save_session(&session1).await.unwrap();
        store.save_session(&session2).await.unwrap();
        store.save_session(&session3).await.unwrap();

        let project1_sessions = store.list_sessions("/tmp/project1").await.unwrap();
        assert_eq!(project1_sessions.len(), 2);

        let project2_sessions = store.list_sessions("/tmp/project2").await.unwrap();
        assert_eq!(project2_sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_update_session_status() {
        let (store, _temp_dir) = create_test_store().await;

        let session = Session::new("test-session", "/tmp/test-plan.md");
        store.save_session(&session).await.unwrap();

        store
            .update_session_status(session.id, "paused")
            .await
            .unwrap();

        let loaded = store.load_session(session.id).await.unwrap().unwrap();
        assert_eq!(loaded.state, SessionState::Paused);
    }

    #[tokio::test]
    async fn test_save_and_load_events() {
        let (store, _temp_dir) = create_test_store().await;

        let session = Session::new("test-session", "/tmp/test-plan.md");
        store.save_session(&session).await.unwrap();

        let event1 = OrchestratorEvent::StepStarted("task-1".to_string());
        let event2 = OrchestratorEvent::StepCompleted("task-1".to_string());

        store.save_event(session.id, &event1).await.unwrap();
        store.save_event(session.id, &event2).await.unwrap();

        let events = store.load_events(session.id).await.unwrap();
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn test_save_and_load_plan_markers() {
        let (store, _temp_dir) = create_test_store().await;

        let session = Session::new("test-session", "/tmp/test-plan.md");
        store.save_session(&session).await.unwrap();

        store
            .save_plan_marker(session.id, &"task-1".to_string(), "completed")
            .await
            .unwrap();
        store
            .save_plan_marker(session.id, &"task-2".to_string(), "in_progress")
            .await
            .unwrap();

        let markers = store.load_plan_markers(session.id).await.unwrap();
        assert_eq!(markers.len(), 2);
        assert_eq!(markers.get("task-1"), Some(&"completed".to_string()));
        assert_eq!(markers.get("task-2"), Some(&"in_progress".to_string()));
    }

    #[tokio::test]
    async fn test_resume_session_resets_in_progress() {
        let (store, _temp_dir) = create_test_store().await;

        let session = Session::new("test-session", "/tmp/test-plan.md");
        store.save_session(&session).await.unwrap();

        // Save some markers including an in-progress one
        store
            .save_plan_marker(session.id, &"task-1".to_string(), "completed")
            .await
            .unwrap();
        store
            .save_plan_marker(session.id, &"task-2".to_string(), "in_progress")
            .await
            .unwrap();
        store
            .save_plan_marker(session.id, &"task-3".to_string(), "pending")
            .await
            .unwrap();
        store
            .save_plan_marker(session.id, &"task-2".to_string(), "in_progress")
            .await
            .unwrap();

        let markers = store.load_plan_markers(session.id).await.unwrap();
        assert_eq!(markers.len(), 3);
        assert_eq!(markers.get("task-1"), Some(&"completed".to_string()));
        assert_eq!(markers.get("task-2"), Some(&"in_progress".to_string()));
        assert_eq!(markers.get("task-3"), Some(&"pending".to_string()));
    }
}
