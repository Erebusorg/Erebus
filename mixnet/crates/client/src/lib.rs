//! The client side of the mixnet: choose a path, build a packet, hand it to the
//! entry node, and wait for an answer over an unrelated return path.
//!
//! The client is the only party that knows both ends of a request, which is why
//! it — not a node, and not a directory — chooses the path, the per-hop delays,
//! and the return path.

pub mod cover;
pub mod rpc;
pub mod sink;

/// The frames a client puts inside a packet. Kept in its own crate so the
/// browser SDK can parse the same bytes.
pub use erebus_envelope as envelope;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use erebus_sphinx::{Packet, PathHop, Surb, SurbSecret};
use erebus_topology::Registry;
use erebus_wire as wire;
use rand::rngs::OsRng;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

use erebus_envelope::{Frame, Reply, Request};

pub struct ClientConfig {
    pub registry: Registry,
    /// Address the client listens on for replies and returning loop probes.
    pub listen: String,
    /// Mean per-hop delay. Larger means a bigger anonymity set and more latency.
    pub mean_delay_ms: f64,
}

struct Pending {
    secret: SurbSecret,
    respond: oneshot::Sender<Vec<u8>>,
}

pub struct Client {
    registry: Registry,
    reply_tag: [u8; 32],
    mean_delay_ms: f64,
    pending: Mutex<HashMap<[u8; 32], Pending>>,
    probes: Mutex<HashMap<[u8; 32], oneshot::Sender<()>>>,
}

impl Client {
    /// Binds the reply listener and returns the client plus its receive loop.
    pub async fn bind(
        config: ClientConfig,
    ) -> Result<(Arc<Self>, impl std::future::Future<Output = ()>)> {
        let listener = TcpListener::bind(&config.listen)
            .await
            .with_context(|| format!("binding {}", config.listen))?;
        let address = listener.local_addr()?.to_string();

        let client = Arc::new(Self {
            registry: config.registry,
            reply_tag: wire::tag_from_address(&address)?,
            mean_delay_ms: config.mean_delay_ms,
            pending: Mutex::new(HashMap::new()),
            probes: Mutex::new(HashMap::new()),
        });

        let receiving = Arc::clone(&client);
        Ok((client, async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let client = Arc::clone(&receiving);
                tokio::spawn(async move {
                    while let Ok(Some(message)) = wire::read_message(&mut stream).await {
                        if let Err(err) = client.accept(&message) {
                            warn!(%err, "dropping delivered frame");
                        }
                    }
                });
            }
        }))
    }

    /// Sends `body` to `destination` and waits for the answer to come back over
    /// a return path the destination never sees.
    pub async fn request(&self, destination: &str, body: &[u8], wait: Duration) -> Result<Vec<u8>> {
        let path = self.registry.select_path(&mut OsRng, self.mean_delay_ms)?;
        let return_path =
            self.registry
                .select_return_path(&mut OsRng, self.mean_delay_ms, &path)?;
        let (surb, secret) = Surb::new(&return_path, self.reply_tag)?;

        let (respond, answer) = oneshot::channel();
        self.pending
            .lock()
            .expect("pending map poisoned")
            .insert(secret.id, Pending { secret, respond });

        let request = Request::new(body.to_vec(), Some(surb));
        self.dispatch(&path, destination, &Frame::Request(request))
            .await?;

        timeout(wait, answer)
            .await
            .map_err(|_| anyhow!("no reply within {wait:?}"))?
            .map_err(|_| anyhow!("reply channel closed"))
    }

    /// Sends `body` with no reply block: nothing can answer, and nothing has to.
    pub async fn send(&self, destination: &str, body: &[u8]) -> Result<()> {
        let path = self.registry.select_path(&mut OsRng, self.mean_delay_ms)?;
        let request = Request::new(body.to_vec(), None);
        self.dispatch(&path, destination, &Frame::Request(request))
            .await
    }

    /// Sends a packet to the client's own address and waits for it to come back.
    ///
    /// A returning probe is evidence that every hop on the path is still
    /// forwarding; a probe that never returns is evidence that one of them is
    /// not, which is what makes silent dropping a detectable offence rather than
    /// a free one.
    pub async fn loop_probe(&self, wait: Duration) -> Result<Duration> {
        let path = self.registry.select_path(&mut OsRng, self.mean_delay_ms)?;
        let mut id = [0u8; 32];
        rand::RngCore::fill_bytes(&mut OsRng, &mut id);

        let (tx, rx) = oneshot::channel();
        self.probes
            .lock()
            .expect("probe map poisoned")
            .insert(id, tx);

        let started = std::time::Instant::now();
        let address = wire::address_from_tag(&self.reply_tag)?;
        self.dispatch(&path, &address, &Frame::Probe { id }).await?;

        timeout(wait, rx)
            .await
            .map_err(|_| anyhow!("probe did not return within {wait:?}"))?
            .map_err(|_| anyhow!("probe channel closed"))?;
        Ok(started.elapsed())
    }

    /// Sends a packet that carries nothing, so that a packet which carries
    /// something is not distinguishable by the fact that it was sent.
    pub async fn send_cover(&self, destination: &str) -> Result<()> {
        let path = self.registry.select_path(&mut OsRng, self.mean_delay_ms)?;
        self.dispatch(&path, destination, &Frame::Request(Request::cover()))
            .await
    }

    async fn dispatch(&self, path: &[PathHop], destination: &str, frame: &Frame) -> Result<()> {
        let packet = Packet::build(
            &frame.to_bytes(),
            path,
            wire::tag_from_address(destination)?,
        )?;
        let entry = self.registry.address_of(&path[0].id)?;
        wire::send_packet(&entry, &packet).await
    }

    /// Handles a frame delivered to the client's own address.
    fn accept(&self, message: &[u8]) -> Result<()> {
        match Frame::from_bytes(message)? {
            Frame::Reply(Reply { surb_id, sealed }) => {
                let pending = self
                    .pending
                    .lock()
                    .expect("pending map poisoned")
                    .remove(&surb_id)
                    .ok_or_else(|| anyhow!("reply for an unknown reply block"))?;
                let body = pending.secret.open(&sealed)?;
                pending
                    .respond
                    .send(body)
                    .map_err(|_| anyhow!("caller stopped waiting for the reply"))
            }
            Frame::Probe { id } => {
                let sender = self
                    .probes
                    .lock()
                    .expect("probe map poisoned")
                    .remove(&id)
                    .ok_or_else(|| anyhow!("probe that was never sent"))?;
                let _ = sender.send(());
                debug!("loop probe returned");
                Ok(())
            }
            Frame::Request(_) => Err(anyhow!("a client is not a destination service")),
        }
    }
}
