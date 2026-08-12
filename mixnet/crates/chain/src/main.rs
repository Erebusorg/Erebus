//! Reads the node set from the registry contract.
//!
//! Useful on its own: an operator can see exactly what clients will see, and a
//! devnet can turn a deployed registry into the JSON file the older tooling
//! takes.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use erebus_chain::ChainRegistry;

#[derive(Parser)]
#[command(name = "erebus-registry", about = "Reads the Erebus node registry")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fetches the node set and the epoch seed.
    Fetch {
        /// JSON-RPC endpoint.
        #[arg(long)]
        rpc: String,
        /// Address of the NodeRegistry contract.
        #[arg(long)]
        contract: String,
        /// Where to write the registry. Defaults to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "erebus_chain=info".into()),
        )
        .init();

    match Cli::parse().command {
        Command::Fetch { rpc, contract, out } => {
            let registry = ChainRegistry::new(rpc, &contract)?.fetch().await?;
            match out {
                Some(path) => registry.save(&path)?,
                None => println!("{}", serde_json::to_string_pretty(&registry)?),
            }
        }
    }

    Ok(())
}
