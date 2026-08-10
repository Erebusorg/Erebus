//! A mix node.
//!
//! One packet arrives, one layer comes off, the packet waits for the delay it
//! was handed, and something bitwise unrelated leaves. The node learns the hop
//! before it, the hop after it, and nothing about the packet's origin,
//! destination, or contents.

pub mod replay;

use std::sync::Arc;

use anyhow::{Context, Result};
use erebus_sphinx::{Packet, PrivateKey, Processed};
use erebus_topology::{encode_id, Registry};
use erebus_wire as wire;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, Duration};
use tracing::{debug, warn};

use replay::ReplayFilter;

pub struct NodeConfig {
    pub key: PrivateKey,
    pub listen: String,
    pub registry: Registry,
}

pub struct MixNode {
    key: PrivateKey,
    registry: Registry,
    replay: ReplayFilter,
}

impl MixNode {
    pub fn new(key: PrivateKey, registry: Registry) -> Self {
        Self {
            key,
            registry,
            replay: ReplayFilter::new(),
        }
    }

    /// Binds `listen` and returns the bound address plus the accept loop.
    ///
    /// The address is returned rather than assumed so a test can bind port 0 and
    /// still build a registry that resolves.
    pub async fn bind(
        config: NodeConfig,
    ) -> Result<(String, impl std::future::Future<Output = ()>)> {
        let listener = TcpListener::bind(&config.listen)
            .await
            .with_context(|| format!("binding {}", config.listen))?;
        let address = listener.local_addr()?.to_string();
        let node = Arc::new(MixNode::new(config.key, config.registry));

        Ok((address, async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let node = Arc::clone(&node);
                        tokio::spawn(async move {
                            if let Err(err) = node.handle(stream).await {
                                debug!(%err, "connection ended");
                            }
                        });
                    }
                    Err(err) => {
                        warn!(%err, "accept failed");
                        return;
                    }
                }
            }
        }))
    }

    async fn handle(self: &Arc<Self>, mut stream: TcpStream) -> Result<()> {
        while let Some(packet) = wire::read_packet(&mut stream).await? {
            let node = Arc::clone(self);
            tokio::spawn(async move {
                if let Err(err) = node.process(packet).await {
                    warn!(%err, "dropping packet");
                }
            });
        }
        Ok(())
    }

    /// Peels a layer, waits, and forwards or delivers.
    pub async fn process(&self, packet: Packet) -> Result<()> {
        if !self.replay.accept(PrivateKey::replay_tag(&packet)) {
            debug!("dropping replayed packet");
            return Ok(());
        }

        match self.key.process(&packet)? {
            Processed::Forward {
                next_id,
                delay_ms,
                packet,
            } => {
                let address = self.registry.address_of(&next_id)?;
                sleep(Duration::from_millis(delay_ms as u64)).await;
                debug!(next = %encode_id(&next_id), delay_ms, "forwarding");
                wire::send_packet(&address, &packet).await
            }
            Processed::Deliver {
                tag,
                delay_ms,
                message,
            } => {
                let address = wire::address_from_tag(&tag)?;
                sleep(Duration::from_millis(delay_ms as u64)).await;
                debug!(%address, delay_ms, bytes = message.len(), "delivering");
                wire::send_message(&address, &message).await
            }
        }
    }
}
