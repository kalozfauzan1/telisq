use clap::Parser;

#[derive(Parser)]
#[command(about = "Show status of current plan")]
pub struct Status {
    /// Show detailed information
    #[arg(short, long)]
    pub detailed: bool,
}

impl Status {
    pub fn run(self) -> anyhow::Result<()> {
        println!("Plan status:");
        println!("  Detailed: {}", self.detailed);

        Ok(())
    }
}
