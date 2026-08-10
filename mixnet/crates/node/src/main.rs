use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use erebus_node::{MixNode, NodeConfig};
use erebus_sphinx::PrivateKey;
use erebus_topology::{decode_id, encode_id, Registry};
use tracing::info;

#[derive(Parser)]
#[command(name = "erebus-node", about = "An Erebus mix node")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generates a node key and prints its public id.
    Keygen {
        /// Where to write the private key, as hex.
        #[arg(long)]
        out: PathBuf,
    },
    /// Runs the node.
    Run {
        /// File holding the node's private key, as hex.
        #[arg(long)]
        key: PathBuf,
        /// Address to listen on.
        #[arg(long, default_value = "0.0.0.0:9000")]
        listen: String,
        /// Registry file describing the node set.
        #[arg(long)]
        registry: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "erebus_node=info".into()),
        )
        .init();

    match Cli::parse().command {
        Command::Keygen { out } => {
            let key = PrivateKey::random();
            std::fs::write(&out, encode_id(&key.to_bytes()))?;
            println!("{}", encode_id(&key.public().to_bytes()));
        }
        Command::Run {
            key,
            listen,
            registry,
        } => {
            let key = PrivateKey::from_bytes(decode_id(
                std::fs::read_to_string(&key)
                    .with_context(|| format!("reading {}", key.display()))?
                    .trim(),
            )?);
            let registry = Registry::load(&registry)?;

            let (address, serve) = MixNode::bind(NodeConfig {
                key,
                listen,
                registry,
            })
            .await?;
            info!(%address, "listening");
            serve.await;
        }
    }

    Ok(())
}
