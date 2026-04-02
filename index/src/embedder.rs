//! Ollama HTTP client for generating embeddings.
//!
//! This module provides an `Embedder` struct that communicates with the Ollama API
//! to generate embedding vectors for text input.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Request payload for Ollama embeddings API
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    prompt: String,
}

/// Response from Ollama embeddings API
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

/// Ollama embedder client with retry logic
pub struct Embedder {
    client: Client,
    ollama_url: String,
    model: String,
    max_retries: u32,
    base_delay: Duration,
}

impl Embedder {
    /// Create a new Embedder instance
    ///
    /// # Arguments
    /// * `ollama_url` - Base URL for Ollama API (e.g., "http://localhost:11434")
    /// * `model` - Model name to use for embeddings (default: "nomic-embed-text")
    pub fn new(ollama_url: &str, model: Option<&str>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            ollama_url: ollama_url.trim_end_matches('/').to_string(),
            model: model.unwrap_or("nomic-embed-text").to_string(),
            max_retries: 3,
            base_delay: Duration::from_millis(500),
        }
    }

    /// Create a new Embedder with custom retry settings
    pub fn with_retries(
        ollama_url: &str,
        model: Option<&str>,
        max_retries: u32,
        base_delay: Duration,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            ollama_url: ollama_url.trim_end_matches('/').to_string(),
            model: model.unwrap_or("nomic-embed-text").to_string(),
            max_retries,
            base_delay,
        }
    }

    /// Generate an embedding vector for a single text input
    ///
    /// # Arguments
    /// * `text` - The text to embed
    ///
    /// # Returns
    /// The embedding vector as `Vec<f32>`
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedderError> {
        debug!("Embedding text ({} chars)", text.len());
        self.embed_with_retry(text).await
    }

    /// Generate embeddings for multiple text chunks in batch
    ///
    /// # Arguments
    /// * `texts` - Slice of texts to embed
    ///
    /// # Returns
    /// Vector of embedding vectors, in the same order as input
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        info!("Embedding {} texts in batch", texts.len());
        let mut embeddings = Vec::with_capacity(texts.len());

        for (i, text) in texts.iter().enumerate() {
            debug!("Embedding chunk {}/{}", i + 1, texts.len());
            let embedding = self.embed_with_retry(text).await?;
            embeddings.push(embedding);
        }

        info!("Batch embedding complete: {} embeddings", embeddings.len());
        Ok(embeddings)
    }

    /// Check if Ollama is reachable
    pub async fn health_check(&self) -> Result<bool, EmbedderError> {
        debug!("Checking Ollama health at {}", self.ollama_url);
        let url = format!("{}/api/tags", self.ollama_url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Ollama is reachable");
                    Ok(true)
                } else {
                    warn!("Ollama returned non-success status: {}", response.status());
                    Ok(false)
                }
            }
            Err(e) => {
                error!("Ollama health check failed: {}", e);
                Err(EmbedderError::ConnectionFailed(e.to_string()))
            }
        }
    }

    /// Internal method with retry logic
    async fn embed_with_retry(&self, text: &str) -> Result<Vec<f32>, EmbedderError> {
        let url = format!("{}/api/embeddings", self.ollama_url);
        let request = EmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };

        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = self.base_delay * 2u32.pow(attempt - 1);
                warn!(
                    "Retry attempt {}/{} after {}ms delay",
                    attempt,
                    self.max_retries,
                    delay.as_millis()
                );
                tokio::time::sleep(delay).await;
            }

            match self.client.post(&url).json(&request).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<EmbeddingResponse>().await {
                            Ok(embedding_response) => {
                                debug!(
                                    "Successfully got embedding with {} dimensions",
                                    embedding_response.embedding.len()
                                );
                                return Ok(embedding_response.embedding);
                            }
                            Err(e) => {
                                error!("Failed to parse embedding response: {}", e);
                                last_error = Some(EmbedderError::ParseError(e.to_string()));
                            }
                        }
                    } else {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        error!("Ollama API error ({}): {}", status, body);
                        last_error = Some(EmbedderError::ApiError {
                            status: status.as_u16(),
                            message: body,
                        });
                    }
                }
                Err(e) => {
                    error!(
                        "Connection error on attempt {}/{}: {}",
                        attempt + 1,
                        self.max_retries + 1,
                        e
                    );
                    last_error = Some(EmbedderError::ConnectionFailed(e.to_string()));
                }
            }
        }

        Err(last_error.unwrap_or(EmbedderError::UnknownError(
            "All retry attempts failed".to_string(),
        )))
    }
}

/// Errors that can occur during embedding
#[derive(Debug, thiserror::Error)]
pub enum EmbedderError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("API error (status {status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Unknown error: {0}")]
    UnknownError(String),
}
