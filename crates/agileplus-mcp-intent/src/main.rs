// SPDX-License-Identifier: MIT OR Apache-2.0
//! Binary entry point for agileplus-mcp-intent.

use std::net::SocketAddr;

use clap::{Parser, Subcommand};

use agileplus_mcp_intent::{
    converter::convert,
    http::{self},
    mcp,
    storage::{open_storage, store_and_summarize},
    types::ConvertOptions,
    validator::validate_and_wrap,
};

#[derive(Parser)]
#[command(
    name = "agileplus-mcp-intent",
    about = "MCP tool + HTTP service for converting user prompts into AgilePlus intent graphs",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the MCP server over stdio (JSON-RPC)
    Mcp,
    /// Run the HTTP API server
    Http {
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(long, default_value = "8080")]
        port: u16,
    },
    /// Convert a single prompt and print the JSON graph
    Convert {
        /// The user prompt
        prompt: String,
        /// Auto-decompose into features
        #[arg(long, default_value_t = true)]
        auto_decompose: bool,
        /// Maximum number of features to generate
        #[arg(long, default_value_t = 5)]
        max_features: usize,
        /// Store the resulting graph in the AgilePlus database
        #[arg(long, default_value_t = false)]
        store: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _subscriber = tracing_subscriber::fmt::try_init();
    let cli = Cli::parse();

    match cli.command {
        Command::Mcp => {
            mcp::run_stdio_server()?;
        }
        Command::Http { host, port } => {
            let addr: SocketAddr = format!("{host}:{port}")
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid bind address: {e}"))?;
            http::start_http(addr).await?;
        }
        Command::Convert {
            prompt,
            auto_decompose,
            max_features,
            store,
        } => {
            let options = ConvertOptions {
                auto_decompose,
                max_features,
                store,
            };
            let response = convert(&prompt, &options)?;
            let validated = match validate_and_wrap(response) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Validation error: {}", e.error);
                    std::process::exit(1);
                }
            };

            if store {
                match open_storage() {
                    Ok(db) => match store_and_summarize(&db, &validated.graph).await {
                        Ok(summary) => {
                            eprintln!("Stored {} feature(s) in database.", summary.features_stored);
                            for id in &summary.ids {
                                eprintln!("  - feature id: {id}");
                            }
                        }
                        Err(e) => {
                            eprintln!("Warning: failed to store graph: {e}");
                        }
                    },
                    Err(e) => {
                        eprintln!("Warning: could not open database for storage: {e}");
                    }
                }
            }

            let json = serde_json::to_string_pretty(&validated.graph)?;
            println!("{json}");
        }
    }

    Ok(())
}
