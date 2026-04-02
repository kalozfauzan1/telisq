use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use mcp::registry::McpRegistry;
use shared::brief::AgentType;
use shared::config::{AppConfig, LlmConfig};
use shared::types::SessionId;
use std::path::PathBuf;
use std::sync::Arc;
use telisq_core::agents::plan_agent::{PlanAgent, PlanAgentConfig};
use telisq_core::agents::{AgentContext, AgentEvent, AgentRunner};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(about = "Manage plans (create, edit, list, validate)")]
pub struct Plan {
    #[command(subcommand)]
    pub command: PlanCommand,
}

#[derive(Subcommand)]
pub enum PlanCommand {
    /// Create a new plan
    Create {
        /// User goal or project description
        #[arg(short, long, value_name = "GOAL")]
        goal: Option<String>,

        /// Profile name to use
        #[arg(short = 'P', long, value_name = "NAME")]
        profile: Option<String>,
    },
    /// Edit an existing plan
    Edit {
        /// Path to plan file
        #[arg(short, long, value_name = "PATH")]
        plan_path: Option<PathBuf>,
    },
    /// List available plans
    List {
        /// Profile name to filter by
        #[arg(short = 'P', long, value_name = "NAME")]
        profile: Option<String>,
    },
    /// Validate a plan file
    Validate {
        /// Path to plan file
        #[arg(short, long, value_name = "PATH")]
        plan_path: Option<PathBuf>,
    },
}

impl Plan {
    pub fn run(self) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        match &self.command {
            PlanCommand::Create { goal, profile } => {
                self.create_plan(goal.clone(), profile.clone()).await
            }
            PlanCommand::Edit { plan_path } => self.edit_plan(plan_path.clone()).await,
            PlanCommand::List { profile } => self.list_plans(profile.clone()).await,
            PlanCommand::Validate { plan_path } => self.validate_plan(plan_path.clone()).await,
        }
    }

    /// Creates a new plan using the Plan Agent
    async fn create_plan(
        &self,
        goal: Option<String>,
        profile: Option<String>,
    ) -> anyhow::Result<()> {
        info!("Creating new plan");

        // Get user goal
        let user_goal = match goal {
            Some(g) => g,
            None => {
                // Interactive prompt for user goal
                println!("Enter your project goal (or press Enter to skip):");
                let mut input = String::new();
                std::io::stdin()
                    .read_line(&mut input)
                    .context("Failed to read user input")?;
                let input = input.trim().to_string();
                if input.is_empty() {
                    return Err(anyhow!("User goal is required for plan creation"));
                }
                input
            }
        };

        info!(user_goal = %user_goal, "Starting plan generation");

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;

        // Initialize MCP registry
        let mcp_registry = McpRegistry::new(config.mcp.servers.clone());
        let failed_servers = mcp_registry.start_all().await;
        if !failed_servers.is_empty() {
            warn!(servers = ?failed_servers, "Some MCP servers failed to start");
        }
        let mcp_registry = Arc::new(mcp_registry);

        // Create Plan Agent
        let plan_agent_config = PlanAgentConfig {
            max_clarification_rounds: 3,
            plans_dir: "plans".to_string(),
            use_mcp_tools: true,
            ambiguity_threshold: 0.8,
            qdrant_top_k: 5,
        };

        let llm_config = LlmConfig {
            base_url: config.llm.base_url.clone(),
            model: config.llm.model.clone(),
            api_key: config.llm.api_key.clone(),
            max_tokens: config.llm.max_tokens,
            temperature: config.llm.temperature,
        };

        let plan_agent = PlanAgent::with_llm("plan_agent", Some(plan_agent_config), llm_config);

        // Create agent context
        let session_id = SessionId::new_v4();
        let context = AgentContext::new(
            session_id,
            None, // No task ID for plan generation
            AgentType::Plan,
            3,     // max_retries
            false, // test_aware
        );

        // Create event channel
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(100);

        // Run plan agent
        info!("Running Plan Agent for goal: {}", user_goal);
        let result = plan_agent.run(context, tx).await;

        // Display result
        match result {
            telisq_core::agents::AgentResult::Success(data) => {
                println!("\nPlan generated successfully!");
                if let Some(plan_path) = data.get("plan_path") {
                    println!("Plan saved to: {}", plan_path);
                }
            }
            telisq_core::agents::AgentResult::Failure(error) => {
                eprintln!("\nPlan generation failed: {}", error);
                return Err(anyhow!("Plan generation failed: {}", error));
            }
            telisq_core::agents::AgentResult::UserInputRequired(question) => {
                println!("\nClarification needed: {}", question);
                return Err(anyhow!("Plan agent needs clarification"));
            }
            _ => {
                eprintln!("\nUnexpected result from Plan Agent");
                return Err(anyhow!("Unexpected result from Plan Agent"));
            }
        }

        info!("Plan creation completed");
        Ok(())
    }

    /// Edits an existing plan
    async fn edit_plan(&self, plan_path: Option<PathBuf>) -> anyhow::Result<()> {
        let path = plan_path.unwrap_or_else(|| PathBuf::from("plans/current.md"));
        if !path.exists() {
            return Err(anyhow!("Plan file not found: {}", path.display()));
        }

        // Open plan in editor
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let status = std::process::Command::new(&editor)
            .arg(&path)
            .status()
            .context("Failed to open editor")?;

        if status.success() {
            println!("Plan updated successfully: {}", path.display());
        } else {
            return Err(anyhow!("Editor exited with non-zero status"));
        }

        Ok(())
    }

    /// Lists available plans
    async fn list_plans(&self, profile: Option<String>) -> anyhow::Result<()> {
        let plans_dir = PathBuf::from("plans");
        if !plans_dir.exists() {
            println!("No plans found (plans/ directory not found)");
            return Ok(());
        }

        let entries = std::fs::read_dir(&plans_dir)?;
        let mut plans: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "md"))
            .collect();

        if plans.is_empty() {
            println!("No plan files found in plans/ directory");
            return Ok(());
        }

        // Sort by modification time (most recent first)
        plans.sort_by(|a, b| {
            let a_time = std::fs::metadata(a).and_then(|m| m.modified()).ok();
            let b_time = std::fs::metadata(b).and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        println!("Available plans:");
        for plan in &plans {
            if let Some(name) = plan.file_stem() {
                let modified = std::fs::metadata(plan)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        chrono::DateTime::<chrono::Utc>::from(t)
                            .format("%Y-%m-%d %H:%M")
                            .to_string()
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                println!("  - {} (modified: {})", name.to_string_lossy(), modified);
            }
        }

        Ok(())
    }

    /// Validates a plan file
    async fn validate_plan(&self, plan_path: Option<PathBuf>) -> anyhow::Result<()> {
        let path = plan_path.unwrap_or_else(|| PathBuf::from("plans/current.md"));
        if !path.exists() {
            return Err(anyhow!("Plan file not found: {}", path.display()));
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read plan file: {}", path.display()))?;

        let tasks = telisq_plan::parser::parse_plan_content(&content)
            .with_context(|| format!("Failed to parse plan file: {}", path.display()))?;

        // Validate task IDs and dependencies
        let mut errors = Vec::new();
        let mut task_ids = std::collections::HashSet::new();

        for task in &tasks {
            if task_ids.contains(&task.id) {
                errors.push(format!("Duplicate task ID: {}", task.id));
            }
            task_ids.insert(task.id.clone());

            // Check dependencies exist
            for dep in &task.dependencies {
                if !task_ids.contains(dep) && !tasks.iter().any(|t| t.id == *dep) {
                    errors.push(format!(
                        "Task '{}' depends on unknown task '{}'",
                        task.id, dep
                    ));
                }
            }
        }

        if errors.is_empty() {
            println!("✅ Plan is valid: {}", path.display());
            println!("   {} tasks found", tasks.len());
        } else {
            println!("❌ Plan validation failed:");
            for error in &errors {
                println!("   - {}", error);
            }
            return Err(anyhow!(
                "Plan validation failed with {} errors",
                errors.len()
            ));
        }

        Ok(())
    }
}
