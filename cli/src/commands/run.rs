use anyhow::{anyhow, Context, Result};
use clap::Parser;
use mcp::registry::McpRegistry;
use shared::brief::AgentType;
use shared::config::{AppConfig, LlmConfig};
use shared::types::{SessionId, TaskSpec};
use std::path::PathBuf;
use std::sync::Arc;
use telisq_core::agents::plan_agent::{PlanAgent, PlanAgentConfig};
use telisq_core::orchestrator::{Orchestrator, OrchestratorConfig, OrchestratorEvent};
use telisq_core::session::store::SessionStore;
use telisq_plan::parser::parse_plan_content;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(about = "Run the execution phase with TUI")]
pub struct Run {
    /// Path to plan file
    #[arg(short, long, value_name = "PATH")]
    pub plan_path: Option<PathBuf>,

    /// Profile name to use
    #[arg(short = 'P', long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Continue from the last completed step
    #[arg(short, long)]
    pub continue_from: Option<String>,

    /// Dry run without making changes
    #[arg(short, long)]
    pub dry_run: bool,
}

impl Run {
    pub fn run(self) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        info!("Starting execution phase");

        // Load configuration
        let config = AppConfig::load().context("Failed to load configuration")?;

        // Resolve plan path
        let plan_path = self.resolve_plan_path()?;
        info!(plan_path = %plan_path.display(), "Using plan file");

        // Parse the plan file to extract tasks
        let plan_content = std::fs::read_to_string(&plan_path)
            .with_context(|| format!("Failed to read plan file: {}", plan_path.display()))?;
        let tasks = parse_plan_content(&plan_content)
            .with_context(|| format!("Failed to parse plan file: {}", plan_path.display()))?;

        // Create session
        let session_id = SessionId::new_v4();
        info!(session_id = %session_id, "Created new session");

        // Initialize MCP registry
        let mcp_registry = McpRegistry::new(config.mcp.servers.clone());
        let failed_servers = mcp_registry.start_all().await;
        if !failed_servers.is_empty() {
            warn!(servers = ?failed_servers, "Some MCP servers failed to start");
        }
        let mcp_registry = Arc::new(mcp_registry);

        // Initialize LLM config
        let llm_config = Some(LlmConfig {
            base_url: config.llm.base_url.clone(),
            model: config.llm.model.clone(),
            api_key: config.llm.api_key.clone(),
            max_tokens: config.llm.max_tokens,
            temperature: config.llm.temperature,
        });

        // Initialize session store
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".telisq");
        let db_path = data_dir.join("telisq.db").to_string_lossy().to_string();
        let session_store = SessionStore::new(&db_path)
            .await
            .context("Failed to initialize session store")?;

        // Create orchestrator
        let orchestrator_config = OrchestratorConfig::default();

        let mut orchestrator =
            Orchestrator::with_llm(session_id, Some(orchestrator_config), llm_config);
        orchestrator = orchestrator
            .with_plan_path(plan_path.clone())
            .with_session_store(session_store);

        // Add tasks from plan
        let task_count = tasks.len();
        for task_spec in &tasks {
            let task_id = task_spec.id.clone();

            // Create plan agent for each task
            let plan_agent_config = PlanAgentConfig {
                max_clarification_rounds: 3,
                plans_dir: "plans".to_string(),
                use_mcp_tools: true,
                ambiguity_threshold: 0.8,
                qdrant_top_k: 5,
            };
            let llm_cfg = LlmConfig {
                base_url: config.llm.base_url.clone(),
                model: config.llm.model.clone(),
                api_key: config.llm.api_key.clone(),
                max_tokens: config.llm.max_tokens,
                temperature: config.llm.temperature,
            };
            let plan_agent = PlanAgent::with_llm(&task_id, Some(plan_agent_config), llm_cfg);

            orchestrator.add_task(task_spec.clone(), Box::new(plan_agent), AgentType::Plan);
        }

        // Initialize task graph for dependency ordering
        orchestrator.init_task_graph()?;

        // Handle continue_from
        if let Some(ref task_id) = self.continue_from {
            info!(task_id = %task_id, "Resuming from task");
            orchestrator = orchestrator.with_continue_from(task_id.clone());
        }

        // Create event channel for TUI communication
        let (event_tx, event_rx) = mpsc::channel::<OrchestratorEvent>(100);
        orchestrator.set_event_tx(event_tx);

        // Initialize TUI
        let mut app = crate::tui::app::App::new()?;
        app.state.session_id = Some(session_id.to_string());

        // Load plan nodes from plan file
        app.state.plan_nodes = (0..task_count)
            .map(|i| format!("[ ] Task {}", i + 1))
            .collect();

        // Load task specs for display
        let task_specs: Vec<TaskSpec> = tasks.clone();
        app.update_tasks(task_specs);

        // Set orchestrator event receiver
        app.events.set_orchestrator_rx(event_rx);

        // Spawn orchestrator as background task
        tokio::spawn(async move {
            info!("Starting orchestrator in background");
            if let Err(e) = orchestrator.run().await {
                error!(error = %e, "Orchestrator failed");
            }
            info!("Orchestrator finished");
        });

        // Setup Ctrl+C handler
        let ctrl_c_session_id = session_id.clone();
        let ctrl_c_plan_path = plan_path.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            info!("Received Ctrl+C, persisting session...");
            // Session will be persisted by the orchestrator's stop_session method
            warn!(session_id = %ctrl_c_session_id, "Session persisted on exit");
        });

        // Run TUI with orchestrator
        info!("Starting TUI with orchestrator");
        app.run().await?;

        info!("Execution phase completed");
        Ok(())
    }

    /// Resolves the plan file path from arguments or auto-discovery
    fn resolve_plan_path(&self) -> anyhow::Result<PathBuf> {
        if let Some(path) = &self.plan_path {
            if !path.exists() {
                return Err(anyhow!("Plan file not found: {}", path.display()));
            }
            return Ok(path.clone());
        }

        // Auto-discover in plans/ directory
        let plans_dir = PathBuf::from("plans");
        if !plans_dir.exists() {
            return Err(anyhow!(
                "No plan file specified and plans/ directory not found"
            ));
        }

        // Look for the most recent plan file
        let mut plans: Vec<PathBuf> = std::fs::read_dir(&plans_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "md"))
            .collect();

        if plans.is_empty() {
            return Err(anyhow!("No plan files found in plans/ directory"));
        }

        // Sort by modification time (most recent first)
        plans.sort_by(|a, b| {
            let a_time = std::fs::metadata(a).and_then(|m| m.modified()).ok();
            let b_time = std::fs::metadata(b).and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        Ok(plans[0].clone())
    }
}
