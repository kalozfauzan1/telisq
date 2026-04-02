//! Index crate for Ollama embeddings and Qdrant vector search.
//!
//! This crate provides functionality for:
//! - Embedding text using Ollama's API
//! - Storing and searching embeddings in Qdrant
//! - Crawling file systems to extract and chunk code
//! - Watching files for changes and updating the index

pub mod crawler;
pub mod embedder;
pub mod store;
pub mod watcher;

// Re-export key types
pub use crawler::Crawler;
pub use embedder::Embedder;
pub use store::QdrantStore;
pub use watcher::FileWatcher;

use serde::{Deserialize, Serialize};

/// Configuration for the index system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// URL for the Ollama API (default: http://localhost:11434)
    pub ollama_url: String,
    /// URL for the Qdrant API (default: http://localhost:6334)
    pub qdrant_url: String,
    /// Name of the Qdrant collection to use
    pub collection_name: String,
    /// Directories to ignore when crawling
    pub ignored_dirs: Vec<String>,
    /// File extensions to index (e.g., ".rs", ".ts", ".js", ".py", ".md")
    pub indexed_extensions: Vec<String>,
    /// Chunk size in tokens (default: 500)
    pub chunk_size: usize,
    /// Overlap between chunks in tokens (default: 50)
    pub chunk_overlap: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            ollama_url: "http://localhost:11434".to_string(),
            qdrant_url: "http://localhost:6334".to_string(),
            collection_name: "telisq_codebase".to_string(),
            ignored_dirs: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
            indexed_extensions: vec![
                ".rs".to_string(),
                ".ts".to_string(),
                ".js".to_string(),
                ".py".to_string(),
                ".md".to_string(),
                ".json".to_string(),
                ".toml".to_string(),
                ".yaml".to_string(),
                ".yml".to_string(),
            ],
            chunk_size: 500,
            chunk_overlap: 50,
        }
    }
}
