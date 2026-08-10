//! A destination service: what sits at the far end of the mixnet.
//!
//! In production this is the thing that talks to Robinhood Chain. Here it is a
//! generic handler, which is the point: the service sees a request body and a
//! reply block, never a client address, and answers by handing a packet back to
//! the entry node named in the reply block.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use erebus_topology::Registry;
use erebus_wire as wire;
use tokio::net::TcpListener;
use tracing::{debug, warn};

use erebus_envelope::{Frame, Reply};

/// An answer in progress. `None` means nothing to send back.
pub type Answer = Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send>>;

/// Answers a request body. Asynchronous because a real destination service
/// forwards the body somewhere else and waits.
pub type Handler = Arc<dyn Fn(Vec<u8>) -> Answer + Send + Sync>;

/// Wraps an answer that needs no waiting.
pub fn immediate(f: impl Fn(&[u8]) -> Option<Vec<u8>> + Send + Sync + 'static) -> Handler {
    Arc::new(move |body| {
        let answer = f(&body);
        Box::pin(async move { answer })
    })
}

pub struct Sink {
    registry: Registry,
    handler: Handler,
}

impl Sink {
    pub fn new(registry: Registry, handler: Handler) -> Self {
        Self { registry, handler }
    }

    pub async fn bind(
        self,
        listen: &str,
    ) -> Result<(String, impl std::future::Future<Output = ()>)> {
        let listener = TcpListener::bind(listen).await?;
        let address = listener.local_addr()?.to_string();
        let sink = Arc::new(self);

        Ok((address, async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let sink = Arc::clone(&sink);
                tokio::spawn(async move {
                    while let Ok(Some(message)) = wire::read_message(&mut stream).await {
                        if let Err(err) = sink.handle(&message).await {
                            warn!(%err, "dropping request");
                        }
                    }
                });
            }
        }))
    }

    async fn handle(&self, message: &[u8]) -> Result<()> {
        let Frame::Request(request) = Frame::from_bytes(message)? else {
            return Err(anyhow!("a destination service only accepts requests"));
        };

        if request.cover {
            debug!("discarding cover traffic");
            return Ok(());
        }

        let Some(answer) = (self.handler)(request.body).await else {
            return Ok(());
        };
        let Some(surb) = request.surb else {
            debug!("request carried no reply block; answer discarded");
            return Ok(());
        };

        let sealed = surb.seal_reply(&answer)?;
        let frame = Frame::Reply(Reply {
            surb_id: surb.id,
            sealed,
        });
        let entry = self.registry.address_of(&surb.first_hop)?;
        let packet = surb.into_packet(&frame.to_bytes())?;

        wire::send_packet(&entry, &packet).await
    }
}
