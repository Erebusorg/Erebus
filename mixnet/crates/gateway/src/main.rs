use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use erebus_gateway::{Gateway, GatewayConfig};
use erebus_topology::Registry;
use tracing::info;

#[derive(Parser)]
#[command(
    name = "erebus-gateway",
    about = "Carries packets between a browser and the Erebus mixnet"
)]
struct Cli {
    /// Address browsers connect to over WebSocket.
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: String,
    /// Address exit nodes deliver replies to.
    #[arg(long, default_value = "127.0.0.1:9200")]
    mix_listen: String,
    /// The reply address handed to clients, when it differs from `--mix-listen`.
    #[arg(long)]
    advertise: Option<String>,
    /// Registry file describing the node set.
    #[arg(long)]
    registry: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "erebus_gateway=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let (ws, mix, serve) = Gateway::bind(GatewayConfig {
        listen: cli.listen,
        mix_listen: cli.mix_listen,
        advertise: cli.advertise,
        registry: Registry::load(&cli.registry)?,
    })
    .await?;

    info!(websocket = %ws, deliveries = %mix, "listening");
    serve.await;
    Ok(())
}
