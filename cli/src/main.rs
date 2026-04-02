use clap::Parser;
use telisq_cli::commands::Cli;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    cli.run()
}
