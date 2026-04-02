use clap::Parser;

pub mod doctor;
pub mod index;
pub mod plan;
pub mod run;
pub mod session;
pub mod status;

#[derive(Parser)]
#[command(name = "telisq")]
#[command(about = "Telisq - AI-powered software delivery engine")]
#[command(version = "0.1.0")]
#[command(long_about = Some("Telisq orchestrates AI agents to deliver software incrementally, following executable plans with tooling integration."))]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser)]
pub enum Commands {
    /// Run the planning phase
    Plan(plan::Plan),
    /// Run the execution phase with TUI
    Run(run::Run),
    /// Index codebase artifacts
    Index(index::Index),
    /// Show status of current plan
    Status(status::Status),
    /// List and manage sessions
    Session(session::Session),
    /// Run diagnostics
    Doctor(doctor::Doctor),
    /// Bootstrap configuration
    Bootstrap,
}

impl Cli {
    pub fn run(self) -> anyhow::Result<()> {
        match self.command {
            Commands::Plan(cmd) => cmd.run(),
            Commands::Run(cmd) => cmd.run(),
            Commands::Index(cmd) => cmd.run(),
            Commands::Status(cmd) => cmd.run(),
            Commands::Session(cmd) => cmd.run(),
            Commands::Doctor(cmd) => cmd.run(),
            Commands::Bootstrap => bootstrap(),
        }
    }
}

fn bootstrap() -> anyhow::Result<()> {
    println!("Bootstrapping Telisq configuration...");
    // Auto-create default config if missing
    let config = shared::config::AppConfig::default();
    let config_path = shared::config::config_path()?;

    if !config_path.exists() {
        config.save()?;
        println!("Created default config at: {:?}", config_path);
    } else {
        println!("Config already exists at: {:?}", config_path);
    }

    println!("Bootstrap complete");
    Ok(())
}
