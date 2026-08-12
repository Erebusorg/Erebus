use anyhow::Result;
use clap::{Parser, Subcommand};
use erebus_chain::RegistrySource;
use erebus_client::rpc::RpcService;
use erebus_client::sink::{immediate, Sink};
use erebus_client::{Client, ClientConfig};
use tokio::time::Duration;
use tracing::info;

#[derive(Parser)]
#[command(
    name = "erebus-client",
    about = "Send traffic through the Erebus mixnet"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sends a message through the mixnet and prints the reply.
    Send {
        #[command(flatten)]
        registry: RegistrySource,
        /// Destination service, `host:port`.
        #[arg(long)]
        to: String,
        #[arg(long)]
        message: String,
        /// Address the client listens on for the reply.
        #[arg(long, default_value = "127.0.0.1:0")]
        listen: String,
        /// Mean per-hop delay in milliseconds.
        #[arg(long, default_value_t = 50.0)]
        mean_delay_ms: f64,
        /// How long to wait for the reply.
        #[arg(long, default_value_t = 10)]
        timeout_secs: u64,
    },
    /// Times a packet routed from the client back to itself.
    Probe {
        #[command(flatten)]
        registry: RegistrySource,
        #[arg(long, default_value = "127.0.0.1:0")]
        listen: String,
        #[arg(long, default_value_t = 50.0)]
        mean_delay_ms: f64,
    },
    /// Prints the payout addresses of one freshly selected path.
    ///
    /// A payer spends a fee note on these three addresses. Selecting a path here
    /// rather than reusing the one a `send` took is deliberate: a fee that named
    /// the exact route of a known packet would be the link the mixnet exists to
    /// break. The registry is public, so anyone can do this without asking a node
    /// anything.
    Payees {
        #[command(flatten)]
        registry: RegistrySource,
        #[arg(long, default_value_t = 50.0)]
        mean_delay_ms: f64,
    },
    /// Runs a destination service that echoes what it is sent.
    Sink {
        #[command(flatten)]
        registry: RegistrySource,
        #[arg(long, default_value = "127.0.0.1:9100")]
        listen: String,
    },
    /// Runs a destination service that forwards JSON-RPC to a chain node.
    Rpc {
        #[command(flatten)]
        registry: RegistrySource,
        #[arg(long, default_value = "127.0.0.1:9100")]
        listen: String,
        /// The chain node this exit forwards to, as a URL.
        #[arg(long)]
        upstream: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "erebus_client=info".into()),
        )
        .init();

    match Cli::parse().command {
        Command::Send {
            registry,
            to,
            message,
            listen,
            mean_delay_ms,
            timeout_secs,
        } => {
            let (client, serve) = Client::bind(ClientConfig {
                registry: registry.load().await?,
                listen,
                mean_delay_ms,
            })
            .await?;
            tokio::spawn(serve);

            let reply = client
                .request(&to, message.as_bytes(), Duration::from_secs(timeout_secs))
                .await?;
            println!("{}", String::from_utf8_lossy(&reply));
        }
        Command::Probe {
            registry,
            listen,
            mean_delay_ms,
        } => {
            let (client, serve) = Client::bind(ClientConfig {
                registry: registry.load().await?,
                listen,
                mean_delay_ms,
            })
            .await?;
            tokio::spawn(serve);

            let elapsed = client.loop_probe(Duration::from_secs(10)).await?;
            println!("probe returned in {} ms", elapsed.as_millis());
        }
        Command::Payees {
            registry,
            mean_delay_ms,
        } => {
            let registry = registry.load().await?;
            let path = registry.select_path(&mut rand::thread_rng(), mean_delay_ms)?;
            println!("{}", registry.payees(&path)?.join(","));
        }
        Command::Sink { registry, listen } => {
            let sink = Sink::new(
                registry.load().await?,
                immediate(|body: &[u8]| {
                    Some(format!("echo: {}", String::from_utf8_lossy(body)).into_bytes())
                }),
            );
            let (address, serve) = sink.bind(&listen).await?;
            info!(%address, "destination service listening");
            serve.await;
        }
        Command::Rpc {
            registry,
            listen,
            upstream,
        } => {
            let sink = Sink::new(
                registry.load().await?,
                RpcService::with_default_methods(upstream.clone()).handler(),
            );
            let (address, serve) = sink.bind(&listen).await?;
            info!(%address, %upstream, "json-rpc exit listening");
            serve.await;
        }
    }

    Ok(())
}
