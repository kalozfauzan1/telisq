//! File system crawler for extracting and chunking source code.
//!
//! This module provides a `Crawler` struct that recursively traverses directories,
//! filters files by extension, and chunks file content into segments.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

/// A chunk of file content with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// File path relative to the crawl root
    pub file_path: PathBuf,
    /// Index of this chunk within the file
    pub chunk_index: usize,
    /// The text content of this chunk
    pub content: String,
    /// Starting line number (1-based, inclusive)
    pub start_line: usize,
    /// Ending line number (1-based, inclusive)
    pub end_line: usize,
}

/// File system crawler with configurable options
pub struct Crawler {
    /// Directories to ignore during crawling
    ignored_dirs: Vec<String>,
    /// File extensions to include (e.g., ".rs", ".ts")
    indexed_extensions: Vec<String>,
    /// Number of tokens/characters per chunk
    chunk_size: usize,
    /// Overlap between consecutive chunks
    chunk_overlap: usize,
}

impl Crawler {
    /// Create a new Crawler instance
    ///
    /// # Arguments
    /// * `ignored_dirs` - Directory names to skip (e.g., ".git", "node_modules")
    /// * `indexed_extensions` - File extensions to include (e.g., ".rs", ".ts")
    /// * `chunk_size` - Characters per chunk
    /// * `chunk_overlap` - Overlap between consecutive chunks
    pub fn new(
        ignored_dirs: Vec<String>,
        indexed_extensions: Vec<String>,
        chunk_size: usize,
        chunk_overlap: usize,
    ) -> Self {
        Self {
            ignored_dirs,
            indexed_extensions,
            chunk_size,
            chunk_overlap,
        }
    }

    /// Crawl a directory and return chunks of file content
    ///
    /// # Arguments
    /// * `root` - Root directory to crawl
    ///
    /// # Returns
    /// Vector of chunks from all matching files
    pub fn crawl(&self, root: &Path) -> Result<Vec<Chunk>, CrawlerError> {
        info!("Crawling directory: {}", root.display());
        let mut all_chunks = Vec::new();

        let walker = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| self.should_include_entry(e));

        for entry in walker {
            let entry = entry.map_err(|e| CrawlerError::walk_error(root.to_path_buf(), e))?;

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();

            // Check file extension
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_with_dot = format!(".{}", ext);
                if !self.indexed_extensions.contains(&ext_with_dot) {
                    debug!(
                        "Skipping file with non-indexed extension: {}",
                        path.display()
                    );
                    continue;
                }
            } else {
                debug!("Skipping file without extension: {}", path.display());
                continue;
            }

            // Read and chunk file
            match self.chunk_file(path, root) {
                Ok(chunks) => {
                    debug!("File {} produced {} chunks", path.display(), chunks.len());
                    all_chunks.extend(chunks);
                }
                Err(e) => {
                    warn!("Failed to chunk file {}: {}", path.display(), e);
                }
            }
        }

        info!(
            "Crawling complete: {} chunks from {}",
            all_chunks.len(),
            root.display()
        );
        Ok(all_chunks)
    }

    /// Check if a directory entry should be included in the crawl
    fn should_include_entry(&self, entry: &walkdir::DirEntry) -> bool {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip ignored directories
        if entry.file_type().is_dir() && self.ignored_dirs.contains(&name) {
            debug!("Skipping ignored directory: {}", name);
            return false;
        }

        true
    }

    /// Chunk a single file into segments
    fn chunk_file(&self, path: &Path, root: &Path) -> Result<Vec<Chunk>, CrawlerError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| CrawlerError::read_error(path.to_path_buf(), e))?;

        if content.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate relative path
        let relative_path = path.strip_prefix(root).unwrap_or(path).to_path_buf();

        // Split content into lines for line number tracking
        let lines: Vec<&str> = content.lines().collect();
        let _total_lines = lines.len();

        // Chunk by characters with overlap
        let chars: Vec<char> = content.chars().collect();
        let total_chars = chars.len();

        if total_chars == 0 {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();
        let mut start = 0;
        let mut chunk_index = 0;

        while start < total_chars {
            let end = (start + self.chunk_size).min(total_chars);

            // Calculate line numbers for this chunk
            let (start_line, end_line) = self.calculate_line_range(&chars, start, end, &lines);

            let chunk_content: String = chars[start..end].iter().collect();

            chunks.push(Chunk {
                file_path: relative_path.clone(),
                chunk_index,
                content: chunk_content,
                start_line,
                end_line,
            });

            chunk_index += 1;

            // Move to next chunk start, accounting for overlap
            if end >= total_chars {
                break;
            }
            start = end.saturating_sub(self.chunk_overlap);

            // Prevent infinite loop if overlap >= chunk_size
            if start >= end {
                start = end;
            }
        }

        Ok(chunks)
    }

    /// Calculate the line range for a character range
    fn calculate_line_range(
        &self,
        chars: &[char],
        start: usize,
        end: usize,
        lines: &[&str],
    ) -> (usize, usize) {
        // Count newlines to determine line numbers
        let mut start_line = 1;
        let mut end_line = 1;

        for (i, &c) in chars.iter().enumerate() {
            if c == '\n' {
                if i < start {
                    start_line += 1;
                }
                if i < end {
                    end_line += 1;
                }
            }
        }

        // Ensure end_line doesn't exceed total lines
        end_line = end_line.min(lines.len());

        (start_line, end_line)
    }
}

/// Errors that can occur during crawling
#[derive(Debug, thiserror::Error)]
pub enum CrawlerError {
    #[error("Failed to walk directory at {path}: {message}")]
    WalkError { path: PathBuf, message: String },

    #[error("Failed to read file {path}: {message}")]
    ReadError { path: PathBuf, message: String },
}

impl CrawlerError {
    /// Create a WalkError from a path and error message
    pub fn walk_error(path: PathBuf, source: impl std::fmt::Display) -> Self {
        Self::WalkError {
            path,
            message: source.to_string(),
        }
    }

    /// Create a ReadError from a path and error message
    pub fn read_error(path: PathBuf, source: impl std::fmt::Display) -> Self {
        Self::ReadError {
            path,
            message: source.to_string(),
        }
    }
}
