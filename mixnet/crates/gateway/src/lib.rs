//! The bridge between a browser and the mixnet.
//!
//! A page in a browser cannot open a raw socket and cannot be dialled, so it
//! cannot hand a packet to an entry node and cannot be the address a reply is
//! delivered to. The gateway does both on its behalf over a WebSocket.
//!
//! What it can see is what the client's own network link could see anyway: that
//! this client is speaking to the mixnet, and which entry node it chose. What it
//! cannot see is the rest of the path, the destination, the request, or the
//! reply — all of that is inside a packet the gateway has no key for, and the
//! reply it forwards is sealed under a key only the client holds.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use erebus_envelope::Frame;
use erebus_sdk::gateway::{decode_expect, decode_send, encode_deliver, encode_hello, EXPECT, SEND};
use erebus_sphinx::{Packet, PACKET_SIZE};
use erebus_topology::{encode_id, Registry};
use erebus_wire as wire;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, warn};

pub struct GatewayConfig {
    /// Where browsers connect, `host:port`.
    pub listen: String,
    /// Where the mixnet delivers replies, `host:port`. Must be reachable by exit
    /// nodes, and must fit in a 32 byte delivery tag.
    pub mix_listen: String,
    /// The address to hand clients as their reply address, when the gateway is
    /// reached at a different name than it binds.
    pub advertise: Option<String>,
    pub registry: Registry,
}

/// Clients waiting for a delivery, keyed by the reply block or probe id they
/// registered. The gateway knows nothing else about them.
#[derive(Default)]
struct Waiting {
    by_id: HashMap<[u8; 32], mpsc::UnboundedSender<Vec<u8>>>,
}

pub struct Gateway {
    registry: Registry,
    registry_json: String,
    tag: String,
    waiting: Mutex<Waiting>,
}

impl Gateway {
    pub fn new(registry: Registry, tag: String) -> Result<Self> {
        wire::tag_from_address(&tag)?;
        let registry_json = serde_json::to_string(&registry)?;
        Ok(Self {
            registry,
            registry_json,
            tag,
            waiting: Mutex::new(Waiting::default()),
        })
    }

    /// Binds both listeners and returns the WebSocket address, the mixnet-facing
    /// address, and the loops that serve them.
    pub async fn bind(
        config: GatewayConfig,
    ) -> Result<(String, String, impl std::future::Future<Output = ()>)> {
        let mix = TcpListener::bind(&config.mix_listen)
            .await
            .with_context(|| format!("binding {}", config.mix_listen))?;
        let mix_address = mix.local_addr()?.to_string();
        let tag = config
            .advertise
            .clone()
            .unwrap_or_else(|| mix_address.clone());

        let sockets = TcpListener::bind(&config.listen)
            .await
            .with_context(|| format!("binding {}", config.listen))?;
        let ws_address = sockets.local_addr()?.to_string();

        let gateway = Arc::new(Gateway::new(config.registry, tag)?);
        let deliveries = Arc::clone(&gateway);

        let serve = async move {
            let mixnet_side = async move {
                loop {
                    let Ok((mut stream, _)) = mix.accept().await else {
                        return;
                    };
                    let gateway = Arc::clone(&deliveries);
                    tokio::spawn(async move {
                        while let Ok(Some(message)) = wire::read_message(&mut stream).await {
                            if let Err(err) = gateway.deliver(&message) {
                                debug!(%err, "dropping delivery");
                            }
                        }
                    });
                }
            };

            let browser_side = async move {
                loop {
                    let Ok((stream, _)) = sockets.accept().await else {
                        return;
                    };
                    let gateway = Arc::clone(&gateway);
                    tokio::spawn(async move {
                        if let Err(err) = gateway.serve_socket(stream).await {
                            debug!(%err, "socket ended");
                        }
                    });
                }
            };

            tokio::join!(mixnet_side, browser_side);
        };

        Ok((ws_address, mix_address, serve))
    }

    /// Hands a frame the mixnet delivered to whichever client registered its id.
    ///
    /// A frame nobody is waiting for is dropped rather than broadcast: the
    /// gateway has no way to tell whose it is, and guessing would hand one
    /// client another client's traffic.
    pub fn deliver(&self, message: &[u8]) -> Result<()> {
        let id = Frame::from_bytes(message)?
            .routing_id()
            .ok_or_else(|| anyhow!("a gateway is not a destination service"))?;

        let sender = self
            .waiting
            .lock()
            .expect("waiting map poisoned")
            .by_id
            .remove(&id)
            .ok_or_else(|| anyhow!("delivery for a client that is not waiting"))?;

        sender
            .send(encode_deliver(message))
            .map_err(|_| anyhow!("the client disconnected before its reply arrived"))
    }

    async fn serve_socket(self: &Arc<Self>, stream: tokio::net::TcpStream) -> Result<()> {
        let socket = tokio_tungstenite::accept_async(stream).await?;
        let (mut sink, mut stream) = socket.split();
        let (to_client, mut from_gateway) = mpsc::unbounded_channel::<Vec<u8>>();

        sink.send(Message::Binary(encode_hello(
            &self.tag,
            &self.registry_json,
        )?))
        .await?;

        let writing = tokio::spawn(async move {
            while let Some(message) = from_gateway.recv().await {
                if sink.send(Message::Binary(message)).await.is_err() {
                    return;
                }
            }
        });

        // Every id this socket registered, so a disconnect does not leave the
        // gateway holding a table of dead clients.
        let mut registered: Vec<[u8; 32]> = Vec::new();

        while let Some(message) = stream.next().await {
            let Message::Binary(message) = message? else {
                continue;
            };
            match message.first() {
                Some(&EXPECT) => match decode_expect(&message) {
                    Ok(id) => {
                        registered.push(id);
                        self.waiting
                            .lock()
                            .expect("waiting map poisoned")
                            .by_id
                            .insert(id, to_client.clone());
                    }
                    Err(err) => warn!(%err, "dropping a registration from a client"),
                },
                Some(&SEND) => {
                    if let Err(err) = self.forward(&message).await {
                        warn!(%err, "dropping packet from a client");
                    }
                }
                _ => warn!("dropping an unrecognised message from a client"),
            }
        }

        let mut waiting = self.waiting.lock().expect("waiting map poisoned");
        for id in registered {
            waiting.by_id.remove(&id);
        }
        drop(waiting);
        writing.abort();
        Ok(())
    }

    /// Writes a client's packet to the entry node the client chose.
    async fn forward(&self, message: &[u8]) -> Result<()> {
        let (first_hop, packet) = decode_send(message).map_err(|e| anyhow!(e.to_string()))?;
        if packet.len() != PACKET_SIZE {
            return Err(anyhow!(
                "a packet is {PACKET_SIZE} bytes, not {}",
                packet.len()
            ));
        }
        let address = self.registry.address_of(&first_hop)?;
        debug!(entry = %encode_id(&first_hop), "forwarding a client packet");
        wire::send_packet(&address, &Packet::from_bytes(packet)?).await
    }
}

#[cfg(test)]
mod tests {
    use erebus_envelope::{Frame, Reply};
    use erebus_topology::Registry;

    use super::*;

    fn gateway() -> Gateway {
        let registry = Registry {
            epoch_seed: "epoch-1".into(),
            nodes: Vec::new(),
        };
        Gateway::new(registry, "127.0.0.1:9200".into()).unwrap()
    }

    fn reply(id: [u8; 32]) -> Vec<u8> {
        Frame::Reply(Reply {
            surb_id: id,
            sealed: vec![1, 2, 3],
        })
        .to_bytes()
    }

    #[test]
    fn a_delivery_goes_to_the_client_that_registered_its_id() {
        let gateway = gateway();
        let (tx, mut rx) = mpsc::unbounded_channel();
        gateway.waiting.lock().unwrap().by_id.insert([4u8; 32], tx);

        gateway.deliver(&reply([4u8; 32])).unwrap();
        let sent = rx.try_recv().unwrap();
        assert_eq!(&sent[1..], reply([4u8; 32]));
        // The id is spent: a second delivery under it has nowhere to go.
        assert!(gateway.deliver(&reply([4u8; 32])).is_err());
    }

    #[test]
    fn a_delivery_nobody_is_waiting_for_is_dropped() {
        assert!(gateway().deliver(&reply([9u8; 32])).is_err());
    }

    #[test]
    fn a_gateway_address_that_does_not_fit_a_delivery_tag_is_refused() {
        let registry = Registry {
            epoch_seed: "epoch-1".into(),
            nodes: Vec::new(),
        };
        assert!(Gateway::new(registry, "a".repeat(33)).is_err());
    }
}
