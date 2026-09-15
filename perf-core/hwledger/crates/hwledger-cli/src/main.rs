use anyhow::Result;
use clap::{Parser, Subcommand};

mod bench;
mod dev;
mod install;
mod open;
mod status;

#[derive(Parser)]
#[command(name = "hwledger", about = "hwLedger CLI — dev servers, benchmarks, and service management")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start all HMR dev servers
    Dev,
    /// Open all services in browser and native apps
    Open,
    /// Run the benchmark suite
    Bench {
        /// Model to benchmark
        #[arg(long, default_value = "mlx-community/Qwen3.5-0.8B-4bit")]
        model: String,
        /// Quantization variant
        #[arg(long, default_value = "4bit", value_parser = ["4bit", "OptiQ-4bit", "8bit", "bf16"])]
        variant: String,
        /// MLX server URL
        #[arg(long, default_value = "http://localhost:8766/v1")]
        mlx_url: String,
        /// Output directory for results
        #[arg(long, default_value = "data/runs/")]
        output: String,
    },
    /// Copy .app bundles to ~/Applications
    Install,
    /// Check status of all services
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dev => dev::run().await,
        Commands::Open => open::run().await,
        Commands::Bench {
            model,
            variant,
            mlx_url,
            output,
        } => bench::run(&model, &variant, &mlx_url, &output).await,
        Commands::Install => install::run().await,
        Commands::Status => status::run().await,
    }
}
