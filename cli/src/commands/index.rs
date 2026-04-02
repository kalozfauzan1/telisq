use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use shared::config::AppConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use telisq_index::crawler::Crawler;
use telisq_index::embedder::Embedder;
use telisq_index::store::{Point, QdrantStore};
use telisq_index::watcher::FileWatcher;
use telisq_index::IndexConfig;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(about = "Index codebase artifacts")]
pub struct Index {
    #[command(subcommand)]
    pub command: IndexCommand,
}

#[derive(Subcommand)]
pub enum IndexCommand {
    /// Build the codebase index
    Build {
        /// Path to codebase root
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,

        /// Force re-index even if already indexed
        #[arg(short, long)]
        force: bool,
    },
    /// Search the codebase index
    Search {
        /// Search query
        #[arg(value_name = "QUERY")]
        query: String,

        /// Number of results to return
        #[arg(short, long, default_value = "10")]
        top_k: usize,
    },
    /// Watch for file changes and update index
    Watch {
        /// Path to codebase root
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Display index status and health
    Status,
}

impl Index {
    pub fn run(self) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        match &self.command {
            IndexCommand::Build { path, force } => self.build_index(path.clone(), *force).await,
            IndexCommand::Search { query, top_k } => self.search_index(query.clone(), *top_k).await,
            IndexCommand::Watch { path } => self.watch_index(path.clone()).await,
            IndexCommand::Status => self.index_status().await,
        }
    }

    /// Builds the codebase index
    async fn build_index(&self, path: Option<PathBuf>, force: bool) -> anyhow::Result<()> {
        info!("Building codebase index");

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;
        let index_config = IndexConfig::default();

        // Resolve codebase path
        let root =
            path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        if !root.exists() {
            return Err(anyhow!("Codebase path not found: {}", root.display()));
        }

        // Initialize components
        let embedder = Arc::new(Embedder::new(&index_config.ollama_url, None));
        let store = Arc::new(QdrantStore::new(
            &index_config.qdrant_url,
            &index_config.collection_name,
            768, // nomic-embed-text dimension
        ));
        let crawler = Arc::new(Crawler::new(
            index_config.ignored_dirs.clone(),
            index_config.indexed_extensions.clone(),
            index_config.chunk_size,
            index_config.chunk_overlap,
        ));

        // Check Ollama connectivity
        info!("Checking Ollama connectivity...");
        match embedder.health_check().await {
            Ok(true) => info!("Ollama is reachable"),
            Ok(false) => {
                warn!("Ollama returned non-success status");
                return Err(anyhow!("Ollama is not reachable"));
            }
            Err(e) => {
                warn!(error = %e, "Ollama is not reachable, embeddings will fail");
                return Err(anyhow!("Ollama is not reachable: {}", e));
            }
        }

        // Check Qdrant connectivity
        info!("Checking Qdrant connectivity...");
        match store.health_check().await {
            Ok(true) => info!("Qdrant is reachable"),
            Ok(false) => {
                warn!("Qdrant returned non-success status");
                return Err(anyhow!("Qdrant is not reachable"));
            }
            Err(e) => {
                warn!(error = %e, "Qdrant is not reachable");
                return Err(anyhow!("Qdrant is not reachable: {}", e));
            }
        }

        // Create collection if needed
        info!("Ensuring collection exists...");
        store
            .create_collection()
            .await
            .context("Failed to create collection")?;

        // Crawl and chunk files
        info!("Crawling codebase at: {}", root.display());
        let chunks = crawler.crawl(&root).context("Failed to crawl codebase")?;
        info!(
            "Found {} chunks from {} files",
            chunks.len(),
            chunks
                .iter()
                .map(|c| &c.file_path)
                .collect::<std::collections::HashSet<_>>()
                .len()
        );

        // Generate embeddings and upsert to Qdrant
        info!("Generating embeddings and upserting to Qdrant...");
        let mut points = Vec::new();
        let mut errors = 0;

        for (i, chunk) in chunks.iter().enumerate() {
            info!("Embedding chunk {}/{}", i + 1, chunks.len());

            match embedder.embed(&chunk.content).await {
                Ok(embedding) => {
                    let mut payload = HashMap::new();
                    payload.insert(
                        "file_path".to_string(),
                        serde_json::json!(chunk.file_path.to_string_lossy()),
                    );
                    payload.insert(
                        "chunk_index".to_string(),
                        serde_json::json!(chunk.chunk_index),
                    );
                    payload.insert(
                        "start_line".to_string(),
                        serde_json::json!(chunk.start_line),
                    );
                    payload.insert("end_line".to_string(), serde_json::json!(chunk.end_line));
                    payload.insert("content".to_string(), serde_json::json!(chunk.content));

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
                    warn!(error = %e, file = %chunk.file_path.display(), chunk = chunk.chunk_index, "Failed to embed chunk");
                    errors += 1;
                }
            }

            // Batch upsert every 100 points
            if points.len() >= 100 {
                let batch: Vec<Point> = points.drain(..).collect();
                if let Err(e) = store.upsert(batch).await {
                    error!(error = %e, "Failed to upsert batch to Qdrant");
                }
            }
        }

        // Upsert remaining points
        if !points.is_empty() {
            if let Err(e) = store.upsert(points).await {
                error!(error = %e, "Failed to upsert final batch to Qdrant");
            }
        }

        println!("\n✅ Index build completed!");
        println!("   Chunks processed: {}", chunks.len());
        println!("   Errors: {}", errors);
        println!("   Collection: {}", index_config.collection_name);

        Ok(())
    }

    /// Searches the codebase index
    async fn search_index(&self, query: String, top_k: usize) -> anyhow::Result<()> {
        info!("Searching codebase for: {}", query);

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;
        let index_config = IndexConfig::default();

        // Initialize components
        let embedder = Embedder::new(&index_config.ollama_url, None);
        let store = QdrantStore::new(&index_config.qdrant_url, &index_config.collection_name, 768);

        // Generate query embedding
        info!("Generating query embedding...");
        let query_embedding = embedder
            .embed(&query)
            .await
            .context("Failed to generate query embedding")?;

        // Search Qdrant
        info!("Searching Qdrant...");
        let results = store
            .search(query_embedding, top_k)
            .await
            .context("Failed to search Qdrant")?;

        // Display results
        if results.is_empty() {
            println!("No results found for: {}", query);
        } else {
            println!("\n🔍 Search results for: {}", query);
            println!("{} results found:\n", results.len());

            for (i, result) in results.iter().enumerate() {
                let file_path = result
                    .payload
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let start_line = result
                    .payload
                    .get("start_line")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let end_line = result
                    .payload
                    .get("end_line")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let content = result
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                println!(
                    "{}. {} (lines {}-{}) [score: {:.3}]",
                    i + 1,
                    file_path,
                    start_line,
                    end_line,
                    result.score
                );
                println!(
                    "   {}",
                    content.lines().take(3).collect::<Vec<_>>().join("\n   ")
                );
                if content.lines().count() > 3 {
                    println!("   ...");
                }
                println!();
            }
        }

        Ok(())
    }

    /// Watches for file changes and updates index
    async fn watch_index(&self, path: Option<PathBuf>) -> anyhow::Result<()> {
        info!("Starting file watcher for live index updates");

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;
        let index_config = IndexConfig::default();

        // Resolve codebase path
        let root =
            path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        if !root.exists() {
            return Err(anyhow!("Codebase path not found: {}", root.display()));
        }

        // Initialize components
        let embedder = Arc::new(Embedder::new(&index_config.ollama_url, None));
        let store = Arc::new(QdrantStore::new(
            &index_config.qdrant_url,
            &index_config.collection_name,
            768,
        ));
        let crawler = Arc::new(Crawler::new(
            index_config.ignored_dirs.clone(),
            index_config.indexed_extensions.clone(),
            index_config.chunk_size,
            index_config.chunk_overlap,
        ));

        // Create file watcher
        let mut watcher = FileWatcher::new(
            root.clone(),
            crawler,
            embedder,
            store,
            Duration::from_secs(2), // 2 second debounce
        );

        println!("👀 Watching for file changes in: {}", root.display());
        println!("Press Ctrl+C to stop watching");

        // Start watcher
        watcher
            .start()
            .await
            .context("Failed to start file watcher")?;

        // Wait for Ctrl+C
        tokio::signal::ctrl_c().await.ok();
        info!("Received Ctrl+C, stopping file watcher");

        // Stop watcher
        watcher.stop().await;
        println!("\n✅ File watcher stopped");

        Ok(())
    }

    /// Displays index status and health
    async fn index_status(&self) -> anyhow::Result<()> {
        info!("Checking index status");

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;
        let index_config = IndexConfig::default();

        println!("📊 Index Status");
        println!("===============");

        // Check Ollama
        let embedder = Embedder::new(&index_config.ollama_url, None);
        match embedder.health_check().await {
            Ok(true) => println!("✅ Ollama: reachable ({})", index_config.ollama_url),
            Ok(false) => println!("❌ Ollama: not reachable ({})", index_config.ollama_url),
            Err(e) => println!("❌ Ollama: error ({})", e),
        }

        // Check Qdrant
        let store = QdrantStore::new(&index_config.qdrant_url, &index_config.collection_name, 768);
        match store.health_check().await {
            Ok(true) => println!("✅ Qdrant: reachable ({})", index_config.qdrant_url),
            Ok(false) => println!("❌ Qdrant: not reachable ({})", index_config.qdrant_url),
            Err(e) => println!("❌ Qdrant: error ({})", e),
        }

        // Check collection
        match store.collection_exists().await {
            Ok(true) => {
                println!("✅ Collection '{}' exists", index_config.collection_name);
                // List collections to show it's there
                match store.list_collections().await {
                    Ok(collections) => {
                        if collections.contains(&index_config.collection_name) {
                            println!("   Collection verified in list");
                        }
                    }
                    Err(e) => warn!(error = %e, "Failed to list collections"),
                }
            }
            Ok(false) => println!(
                "❌ Collection '{}' does not exist",
                index_config.collection_name
            ),
            Err(e) => println!("❌ Failed to check collection: {}", e),
        }

        // Configuration summary
        println!("\n📋 Configuration:");
        println!("   Ollama URL: {}", index_config.ollama_url);
        println!("   Qdrant URL: {}", index_config.qdrant_url);
        println!("   Collection: {}", index_config.collection_name);
        println!("   Chunk size: {} tokens", index_config.chunk_size);
        println!("   Chunk overlap: {} tokens", index_config.chunk_overlap);
        println!(
            "   Indexed extensions: {}",
            index_config.indexed_extensions.join(", ")
        );
        println!(
            "   Ignored directories: {}",
            index_config.ignored_dirs.join(", ")
        );

        Ok(())
    }
}
