use anyhow::{anyhow, Context, Result};
use clap::Parser;
use mcp::registry::McpRegistry;
use reqwest::Client;
use serde_json::json;
use shared::config::{AppConfig, LlmConfig, McpServerConfig};
use std::process::Command;
use std::time::Duration;
use telisq_index::embedder::Embedder;
use telisq_index::store::QdrantStore;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(about = "Run diagnostics")]
pub struct Doctor {
    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

impl Doctor {
    pub fn run(self) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        println!("🔍 Telisq Diagnostics");
        println!("=====================\n");

        let mut all_passed = true;

        // Check Rust toolchain
        print!("Checking Rust toolchain... ");
        match self.check_rust_toolchain().await {
            Ok(version) => println!("✅ {}", version),
            Err(e) => {
                println!("❌ {}", e);
                all_passed = false;
            }
        }

        // Check Node.js
        print!("Checking Node.js... ");
        match self.check_nodejs().await {
            Ok(version) => println!("✅ {}", version),
            Err(e) => {
                println!("❌ {}", e);
                all_passed = false;
            }
        }

        // Check OPENAI_API_KEY
        print!("Checking OPENAI_API_KEY... ");
        match self.check_openai_api_key().await {
            Ok(_) => println!("✅ Set"),
            Err(e) => {
                println!("❌ {}", e);
                all_passed = false;
            }
        }

        // Load configuration for further checks
        let config = match AppConfig::load() {
            Ok(c) => c,
            Err(e) => {
                println!("\n❌ Failed to load configuration: {}", e);
                return Err(anyhow!("Configuration check failed"));
            }
        };

        // Check Ollama
        print!("Checking Ollama... ");
        match self.check_ollama(&config).await {
            Ok(msg) => println!("✅ {}", msg),
            Err(e) => {
                println!("❌ {}", e);
                all_passed = false;
            }
        }

        // Check Qdrant
        print!("Checking Qdrant... ");
        match self.check_qdrant(&config).await {
            Ok(msg) => println!("✅ {}", msg),
            Err(e) => {
                println!("❌ {}", e);
                all_passed = false;
            }
        }

        // Check MCP servers
        print!("Checking MCP servers... ");
        match self.check_mcp_servers(&config).await {
            Ok(msg) => println!("✅ {}", msg),
            Err(e) => {
                println!("❌ {}", e);
                all_passed = false;
            }
        }

        // Check LLM connectivity
        print!("Checking LLM connectivity... ");
        match self.check_llm_connectivity(&config).await {
            Ok(msg) => println!("✅ {}", msg),
            Err(e) => {
                println!("❌ {}", e);
                all_passed = false;
            }
        }

        println!();
        if all_passed {
            println!("✅ All checks passed!");
        } else {
            println!("⚠️  Some checks failed. Please review the errors above.");
        }

        Ok(())
    }

    /// Checks Rust toolchain version
    async fn check_rust_toolchain(&self) -> Result<String> {
        let output = Command::new("rustc")
            .arg("--version")
            .output()
            .context("Failed to execute rustc")?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout);
            Ok(version.trim().to_string())
        } else {
            Err(anyhow!("rustc returned non-zero exit code"))
        }
    }

    /// Checks Node.js version
    async fn check_nodejs(&self) -> Result<String> {
        let output = Command::new("node")
            .arg("--version")
            .output()
            .context("Failed to execute node")?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout);
            Ok(version.trim().to_string())
        } else {
            Err(anyhow!("node not found or returned non-zero exit code"))
        }
    }

    /// Checks OPENAI_API_KEY is set
    async fn check_openai_api_key(&self) -> Result<()> {
        match std::env::var("OPENAI_API_KEY") {
            Ok(key) if !key.is_empty() => Ok(()),
            _ => Err(anyhow!("OPENAI_API_KEY not set or empty")),
        }
    }

    /// Checks Ollama is reachable and nomic-embed-text model is available
    async fn check_ollama(&self, config: &AppConfig) -> Result<String> {
        let ollama_url = format!("{}/api/tags", config.llm.base_url.replace("v1", ""));
        let client = Client::builder().timeout(Duration::from_secs(5)).build()?;

        let response = client.get(&ollama_url).send().await;
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    // Check if nomic-embed-text model is available
                    let body: serde_json::Value = resp.json().await?;
                    let models = body
                        .get("models")
                        .and_then(|m| m.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    if models.iter().any(|m| m.contains("nomic-embed-text")) {
                        Ok(format!("reachable, nomic-embed-text available"))
                    } else {
                        Ok(format!(
                            "reachable, but nomic-embed-text not found (available: {})",
                            models.join(", ")
                        ))
                    }
                } else {
                    Err(anyhow!("Ollama returned status {}", resp.status()))
                }
            }
            Err(e) => Err(anyhow!("Ollama not reachable: {}", e)),
        }
    }

    /// Checks Qdrant is reachable
    async fn check_qdrant(&self, config: &AppConfig) -> Result<String> {
        let store = QdrantStore::new("http://localhost:6334", "telisq_codebase", 768);
        match store.health_check().await {
            Ok(true) => Ok("reachable".to_string()),
            Ok(false) => Err(anyhow!("Qdrant not reachable (non-success response)")),
            Err(e) => Err(anyhow!("Qdrant not reachable: {}", e)),
        }
    }

    /// Checks MCP servers via registry
    async fn check_mcp_servers(&self, config: &AppConfig) -> Result<String> {
        if config.mcp.servers.is_empty() {
            return Ok("no servers configured".to_string());
        }

        let registry = McpRegistry::new(config.mcp.servers.clone());
        let failed = registry.start_all().await;

        if failed.is_empty() {
            Ok(format!("{} servers started", config.mcp.servers.len()))
        } else {
            Err(anyhow!("Failed to start servers: {}", failed.join(", ")))
        }
    }

    /// Checks LLM connectivity via POST to /chat/completions
    async fn check_llm_connectivity(&self, config: &AppConfig) -> Result<String> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

        let url = format!(
            "{}/chat/completions",
            config.llm.base_url.trim_end_matches('/')
        );

        let request_body = json!({
            "model": config.llm.model,
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 10
        });

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", config.llm.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    Ok(format!("connected to {}", config.llm.model))
                } else {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    Err(anyhow!("LLM returned status {}: {}", status, body))
                }
            }
            Err(e) => Err(anyhow!("Failed to connect to LLM: {}", e)),
        }
    }
}
