//! Qdrant REST client for vector storage and search.
//!
//! This module provides a `QdrantStore` struct that communicates with the Qdrant API
//! to store, search, and manage embedding vectors.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// A point in the vector store with its payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    /// Unique identifier for the point
    pub id: String,
    /// The embedding vector
    pub vector: Vec<f32>,
    /// Associated metadata payload
    pub payload: HashMap<String, serde_json::Value>,
}

/// A scored point returned from search results
#[derive(Debug, Clone, Deserialize)]
pub struct ScoredPoint {
    /// Unique identifier for the point
    pub id: String,
    /// Similarity score (higher = more similar)
    pub score: f32,
    /// The embedding vector (may be omitted if not requested)
    pub vector: Option<Vec<f32>>,
    /// Associated metadata payload
    pub payload: HashMap<String, serde_json::Value>,
}

/// Qdrant vector store client
pub struct QdrantStore {
    client: Client,
    qdrant_url: String,
    collection_name: String,
    vector_size: usize,
}

impl QdrantStore {
    /// Create a new QdrantStore instance
    ///
    /// # Arguments
    /// * `qdrant_url` - Base URL for Qdrant API (e.g., "http://localhost:6334")
    /// * `collection_name` - Name of the collection to use
    /// * `vector_size` - Dimension of vectors (768 for nomic-embed-text)
    pub fn new(qdrant_url: &str, collection_name: &str, vector_size: usize) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            qdrant_url: qdrant_url.trim_end_matches('/').to_string(),
            collection_name: collection_name.to_string(),
            vector_size,
        }
    }

    /// Create the collection in Qdrant if it doesn't exist
    pub async fn create_collection(&self) -> Result<(), StoreError> {
        let url = format!("{}/collections/{}", self.qdrant_url, self.collection_name);

        // Check if collection already exists
        if self.collection_exists().await? {
            info!("Collection '{}' already exists", self.collection_name);
            return Ok(());
        }

        let request = CreateCollectionRequest {
            vectors: VectorsConfig {
                size: self.vector_size,
                distance: "Cosine".to_string(),
            },
        };

        debug!(
            "Creating collection '{}' with vector size {}",
            self.collection_name, self.vector_size
        );
        let response = self.client.put(&url).json(&request).send().await?;

        if response.status().is_success() {
            info!("Collection '{}' created successfully", self.collection_name);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Failed to create collection ({}): {}", status, body);
            Err(StoreError::ApiError {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// Check if collection exists
    pub async fn collection_exists(&self) -> Result<bool, StoreError> {
        let url = format!("{}/collections/{}", self.qdrant_url, self.collection_name);

        let response = self.client.get(&url).send().await?;
        Ok(response.status().is_success())
    }

    /// Upsert points into the collection
    ///
    /// # Arguments
    /// * `points` - Vector of points to insert or update
    pub async fn upsert(&self, points: Vec<Point>) -> Result<(), StoreError> {
        if points.is_empty() {
            debug!("No points to upsert");
            return Ok(());
        }

        let url = format!(
            "{}/collections/{}/points?wait=true",
            self.qdrant_url, self.collection_name
        );

        let request = UpsertRequest { points };
        debug!("Upserting {} points", request.points.len());

        let response = self.client.put(&url).json(&request).send().await?;

        if response.status().is_success() {
            info!("Successfully upserted {} points", request.points.len());
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Failed to upsert points ({}): {}", status, body);
            Err(StoreError::ApiError {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// Search for similar points by query vector
    ///
    /// # Arguments
    /// * `query_vector` - The query embedding vector
    /// * `limit` - Maximum number of results to return
    pub async fn search(
        &self,
        query_vector: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<ScoredPoint>, StoreError> {
        let url = format!(
            "{}/collections/{}/points/search",
            self.qdrant_url, self.collection_name
        );

        let request = SearchRequest {
            vector: query_vector,
            limit,
            with_payload: true,
            with_vector: false,
        };

        debug!("Searching with limit={}", limit);
        let response = self.client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            let search_response: SearchResponse = response.json().await?;
            info!("Search returned {} results", search_response.result.len());
            Ok(search_response.result)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Search failed ({}): {}", status, body);
            Err(StoreError::ApiError {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// Delete the collection
    pub async fn delete_collection(&self) -> Result<(), StoreError> {
        let url = format!("{}/collections/{}", self.qdrant_url, self.collection_name);

        debug!("Deleting collection '{}'", self.collection_name);
        let response = self.client.delete(&url).send().await?;

        if response.status().is_success() {
            info!("Collection '{}' deleted", self.collection_name);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Failed to delete collection ({}): {}", status, body);
            Err(StoreError::ApiError {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// List all collections
    pub async fn list_collections(&self) -> Result<Vec<String>, StoreError> {
        let url = format!("{}/collections", self.qdrant_url);

        debug!("Listing collections");
        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let list_response: CollectionsResponse = response.json().await?;
            let names: Vec<String> = list_response
                .result
                .collections
                .into_iter()
                .map(|c| c.name)
                .collect();
            debug!("Found {} collections", names.len());
            Ok(names)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Failed to list collections ({}): {}", status, body);
            Err(StoreError::ApiError {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// Delete specific points by IDs
    ///
    /// # Arguments
    /// * `ids` - Vector of point IDs to delete
    pub async fn delete_points(&self, ids: Vec<String>) -> Result<(), StoreError> {
        if ids.is_empty() {
            return Ok(());
        }

        let url = format!(
            "{}/collections/{}/points/delete",
            self.qdrant_url, self.collection_name
        );

        let request = DeletePointsRequest {
            points: ids.clone(),
        };

        debug!("Deleting {} points", ids.len());
        let response = self.client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            info!("Successfully deleted {} points", ids.len());
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Failed to delete points ({}): {}", status, body);
            Err(StoreError::ApiError {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// Check if Qdrant is reachable
    pub async fn health_check(&self) -> Result<bool, StoreError> {
        debug!("Checking Qdrant health at {}", self.qdrant_url);
        let url = format!("{}/", self.qdrant_url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Qdrant is reachable");
                    Ok(true)
                } else {
                    warn!("Qdrant returned non-success status: {}", response.status());
                    Ok(false)
                }
            }
            Err(e) => {
                error!("Qdrant health check failed: {}", e);
                Err(StoreError::ConnectionFailed(e.to_string()))
            }
        }
    }
}

// Request/Response types for Qdrant API

#[derive(Debug, Serialize)]
struct CreateCollectionRequest {
    vectors: VectorsConfig,
}

#[derive(Debug, Serialize)]
struct VectorsConfig {
    size: usize,
    distance: String,
}

#[derive(Debug, Serialize)]
struct UpsertRequest {
    points: Vec<Point>,
}

#[derive(Debug, Serialize)]
struct SearchRequest {
    vector: Vec<f32>,
    limit: usize,
    with_payload: bool,
    with_vector: bool,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    result: Vec<ScoredPoint>,
}

#[derive(Debug, Deserialize)]
struct CollectionsResponse {
    result: CollectionsResult,
}

#[derive(Debug, Deserialize)]
struct CollectionsResult {
    collections: Vec<CollectionInfo>,
}

#[derive(Debug, Deserialize)]
struct CollectionInfo {
    name: String,
}

#[derive(Debug, Serialize)]
struct DeletePointsRequest {
    points: Vec<String>,
}

/// Errors that can occur during store operations
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("API error (status {status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Collection not found: {0}")]
    CollectionNotFound(String),
}
