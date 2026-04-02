//! File system watcher for detecting and reacting to file changes.
//!
//! This module provides a `FileWatcher` struct that uses the `notify` crate
//! to watch for file system changes and trigger re-indexing.

use crate::crawler::{Chunk, Crawler};
use crate::embedder::Embedder;
use crate::store::{Point, QdrantStore};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// File watcher with debouncing and incremental updates
pub struct FileWatcher {
    /// Root directory being watched
    root: PathBuf,
    /// Crawler for chunking files
    crawler: Arc<Crawler>,
    /// Embedder for generating embeddings
    embedder: Arc<Embedder>,
    /// Store for persisting embeddings
    store: Arc<QdrantStore>,
    /// Debounce duration
    debounce: Duration,
    /// Watcher instance
    watcher: Option<Arc<Mutex<RecommendedWatcher>>>,
    /// Track which files are pending re-indexing
    pending_changes: Arc<Mutex<HashSet<PathBuf>>>,
    /// Track if the watcher is running
    is_running: Arc<Mutex<bool>>,
}

impl FileWatcher {
    /// Create a new FileWatcher instance
    ///
    /// # Arguments
    /// * `root` - Root directory to watch
    /// * `crawler` - Crawler for chunking files
    /// * `embedder` - Embedder for generating embeddings
    /// * `store` - Store for persisting embeddings
    /// * `debounce` - Debounce duration for file events
    pub fn new(
        root: PathBuf,
        crawler: Arc<Crawler>,
        embedder: Arc<Embedder>,
        store: Arc<QdrantStore>,
        debounce: Duration,
    ) -> Self {
        Self {
            root,
            crawler,
            embedder,
            store,
            debounce,
            watcher: None,
            pending_changes: Arc::new(Mutex::new(HashSet::new())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Start watching for file changes
    pub async fn start(&mut self) -> Result<(), WatcherError> {
        info!("Starting file watcher for: {}", self.root.display());

        let pending = Arc::clone(&self.pending_changes);
        let is_running = Arc::clone(&self.is_running);
        let is_running_for_set = Arc::clone(&self.is_running);

        // Create the watcher
        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                let pending = Arc::clone(&pending);
                let is_running = Arc::clone(&is_running);

                tokio::spawn(async move {
                    if let Ok(event) = result {
                        // Check if watcher is running
                        let running = *is_running.lock().await;
                        if !running {
                            return;
                        }

                        // Handle different event types
                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) => {
                                for path in event.paths {
                                    if path.is_file() {
                                        debug!("File modified/created: {}", path.display());
                                        pending.lock().await.insert(path);
                                    }
                                }
                            }
                            EventKind::Remove(_) => {
                                for path in event.paths {
                                    if path.is_file() {
                                        debug!("File removed: {}", path.display());
                                        pending.lock().await.insert(path);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                });
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )
        .map_err(|e| WatcherError::NotifyError(e.to_string()))?;

        // Watch the root directory recursively
        watcher
            .watch(&self.root, RecursiveMode::Recursive)
            .map_err(|e| WatcherError::NotifyError(e.to_string()))?;

        *is_running_for_set.lock().await = true;
        self.watcher = Some(Arc::new(Mutex::new(watcher)));

        info!("File watcher started successfully");
        Ok(())
    }

    /// Stop watching for file changes
    pub async fn stop(&mut self) -> Result<(), WatcherError> {
        info!("Stopping file watcher");
        *self.is_running.lock().await = false;

        if let Some(watcher) = self.watcher.take() {
            let mut w = watcher.lock().await;
            w.unwatch(&self.root)
                .map_err(|e| WatcherError::NotifyError(e.to_string()))?;
        }

        info!("File watcher stopped");
        Ok(())
    }

    /// Process pending changes with debouncing
    /// This should be called periodically
    pub async fn process_changes(&self) -> Result<ProcessingResult, WatcherError> {
        // Wait for debounce period
        sleep(self.debounce).await;

        let mut pending = self.pending_changes.lock().await;
        if pending.is_empty() {
            return Ok(ProcessingResult::no_changes());
        }

        let files_to_process: Vec<PathBuf> = pending.drain().collect();
        drop(pending);

        info!("Processing {} pending file changes", files_to_process.len());

        let mut result = ProcessingResult {
            updated: 0,
            deleted: 0,
            errors: Vec::new(),
        };

        for file_path in files_to_process {
            if file_path.exists() {
                // File exists - re-chunk and upsert
                match self.update_file(&file_path).await {
                    Ok(count) => {
                        result.updated += count;
                    }
                    Err(e) => {
                        error!("Failed to update file {}: {}", file_path.display(), e);
                        result.errors.push((file_path.clone(), e.to_string()));
                    }
                }
            } else {
                // File deleted - remove from store
                match self.delete_file(&file_path).await {
                    Ok(count) => {
                        result.deleted += count;
                    }
                    Err(e) => {
                        error!("Failed to delete file {}: {}", file_path.display(), e);
                        result.errors.push((file_path.clone(), e.to_string()));
                    }
                }
            }
        }

        info!(
            "Processing complete: {} updated, {} deleted, {} errors",
            result.updated,
            result.deleted,
            result.errors.len()
        );

        Ok(result)
    }

    /// Update a single file in the index
    async fn update_file(&self, file_path: &Path) -> Result<usize, WatcherError> {
        debug!("Updating file: {}", file_path.display());

        // Check if file extension is indexable
        if let Some(_ext) = file_path.extension().and_then(|e| e.to_str()) {
            // We need to check against crawler's extensions
            // For now, we'll try to chunk and let it fail silently
        }

        // Chunk the file
        let chunks = self
            .crawler
            .crawl(file_path.parent().unwrap_or(&self.root))?;

        // Filter chunks for this specific file
        let file_chunks: Vec<&Chunk> = chunks
            .iter()
            .filter(|c| c.file_path == file_path.strip_prefix(&self.root).unwrap_or(file_path))
            .collect();

        if file_chunks.is_empty() {
            debug!("No chunks to update for {}", file_path.display());
            return Ok(0);
        }

        // Generate embeddings for each chunk
        let mut points = Vec::new();
        for chunk in file_chunks {
            match self.embedder.embed(&chunk.content).await {
                Ok(embedding) => {
                    let mut payload = HashMap::new();
                    payload.insert(
                        "file_path".to_string(),
                        serde_json::Value::String(chunk.file_path.to_string_lossy().to_string()),
                    );
                    payload.insert(
                        "chunk_index".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(chunk.chunk_index)),
                    );
                    payload.insert(
                        "start_line".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(chunk.start_line)),
                    );
                    payload.insert(
                        "end_line".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(chunk.end_line)),
                    );

                    let point_id = format!(
                        "{}:{}",
                        chunk.file_path.to_string_lossy(),
                        chunk.chunk_index
                    );

                    points.push(Point {
                        id: point_id,
                        vector: embedding,
                        payload,
                    });
                }
                Err(e) => {
                    warn!("Failed to embed chunk: {}", e);
                }
            }
        }

        // Upsert points
        let point_count = points.len();
        if !points.is_empty() {
            self.store.upsert(points).await?;
        }

        Ok(point_count)
    }

    /// Delete a file's chunks from the index
    async fn delete_file(&self, file_path: &Path) -> Result<usize, WatcherError> {
        debug!("Deleting file from index: {}", file_path.display());

        // Get relative path
        let relative_path = file_path.strip_prefix(&self.root).unwrap_or(file_path);

        // We need to find all points for this file
        // For now, we'll use a naming convention for point IDs
        // In a real implementation, we'd query Qdrant for matching points

        // Generate point IDs based on file path
        // This is a simplification - ideally we'd query for existing points
        let mut ids_to_delete = Vec::new();

        // Try to find existing chunks by attempting common indices
        // This is a workaround - a proper implementation would use Qdrant's filter API
        for i in 0..100 {
            // Assume max 100 chunks per file
            let point_id = format!("{}:{}", relative_path.to_string_lossy(), i);
            ids_to_delete.push(point_id);
        }

        if !ids_to_delete.is_empty() {
            let count = ids_to_delete.len();
            // Delete points (Qdrant will ignore non-existent IDs)
            self.store.delete_points(ids_to_delete).await?;
            Ok(count)
        } else {
            Ok(0)
        }
    }
}

/// Result of processing file changes
#[derive(Debug, Default)]
pub struct ProcessingResult {
    /// Number of chunks updated
    pub updated: usize,
    /// Number of chunks deleted
    pub deleted: usize,
    /// Errors encountered during processing
    pub errors: Vec<(PathBuf, String)>,
}

impl ProcessingResult {
    /// Check if there were any changes
    pub fn has_changes(&self) -> bool {
        self.updated > 0 || self.deleted > 0
    }

    /// Create a result indicating no changes
    pub fn no_changes() -> Self {
        Self::default()
    }
}

/// Errors that can occur during file watching
#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("Notify error: {0}")]
    NotifyError(String),

    #[error("Crawler error: {0}")]
    CrawlerError(#[from] crate::crawler::CrawlerError),

    #[error("Embedder error: {0}")]
    EmbedderError(#[from] crate::embedder::EmbedderError),

    #[error("Store error: {0}")]
    StoreError(#[from] crate::store::StoreError),
}
