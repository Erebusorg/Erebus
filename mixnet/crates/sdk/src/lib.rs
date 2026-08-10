//! The client core, without a network stack.
//!
//! Everything a client does that must not be delegated — picking the path,
//! picking the delays, building the packet, building the reply block, opening
//! the reply — happens here, and none of it touches a socket. That is what
//! makes the same code run in a browser: the transport is somebody else's
//! problem, and a transport that can read the packet would defeat the point of
//! having one.
//!
//! ```no_run
//! use erebus_sdk::MixClient;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let registry_json = "";
//! // The registry comes from the chain, or from whoever the client trusts to
//! // describe the node set; the gateway is merely where replies are addressed.
//! let mut client = MixClient::new(registry_json, 50.0, "127.0.0.1:9200")?;
//! let outgoing = client.request("127.0.0.1:9100", b"{\"method\":\"eth_chainId\"}")?;
//! // `outgoing.packet()` goes to `outgoing.first_hop()`, by any means at all.
//! # Ok(())
//! # }
//! ```

pub mod gateway;

use std::collections::{HashMap, HashSet};

use erebus_envelope::{Frame, Reply, Request};
use erebus_sphinx::{Packet, PathHop, Surb, SurbSecret};
use erebus_topology::{decode_id, Registry};
use rand::rngs::OsRng;
use rand::RngCore;
use thiserror::Error;
use wasm_bindgen::prelude::*;

#[derive(Debug, Error)]
pub enum SdkError {
    #[error("registry: {0}")]
    Registry(String),
    #[error("packet: {0}")]
    Packet(String),
    #[error("the gateway tag `{0}` does not fit in a 32 byte delivery tag")]
    GatewayTag(String),
    #[error("a reply arrived for a request this client never sent")]
    UnknownReply,
    #[error("a destination service cannot deliver a request to a client")]
    RequestToClient,
    #[error("frame: {0}")]
    Frame(String),
}

impl From<SdkError> for JsValue {
    fn from(err: SdkError) -> Self {
        JsValue::from_str(&err.to_string())
    }
}

/// A packet ready to be handed to the gateway, and the id of whatever it is
/// waiting for.
#[wasm_bindgen]
pub struct Outgoing {
    first_hop: Vec<u8>,
    packet: Vec<u8>,
    id: Option<Vec<u8>>,
}

#[wasm_bindgen]
impl Outgoing {
    /// The node the gateway must hand this packet to. Every later hop is
    /// encrypted, including from the gateway.
    #[wasm_bindgen(getter)]
    pub fn first_hop(&self) -> Vec<u8> {
        self.first_hop.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn packet(&self) -> Vec<u8> {
        self.packet.clone()
    }

    /// The reply block or probe id to expect an answer under, if this packet
    /// asked for one.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> Option<Vec<u8>> {
        self.id.clone()
    }
}

/// What came back out of the mixnet.
#[wasm_bindgen]
pub struct Delivery {
    id: Vec<u8>,
    body: Option<Vec<u8>>,
}

#[wasm_bindgen]
impl Delivery {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> Vec<u8> {
        self.id.clone()
    }

    /// The opened reply, or nothing when the delivery was a returning probe.
    #[wasm_bindgen(getter)]
    pub fn body(&self) -> Option<Vec<u8>> {
        self.body.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn is_probe(&self) -> bool {
        self.body.is_none()
    }
}

/// The client. Holds the registry, the reply-block secrets of requests still in
/// flight, and nothing else worth stealing.
#[wasm_bindgen]
pub struct MixClient {
    registry: Registry,
    mean_delay_ms: f64,
    reply_tag: [u8; 32],
    pending: HashMap<[u8; 32], SurbSecret>,
    probes: HashSet<[u8; 32]>,
}

#[wasm_bindgen]
impl MixClient {
    /// `gateway_tag` is the `host:port` the gateway receives mixnet deliveries
    /// on, which is where this client's replies are addressed. The browser is
    /// not reachable, so the gateway is reachable on its behalf.
    #[wasm_bindgen(constructor)]
    pub fn new(
        registry_json: &str,
        mean_delay_ms: f64,
        gateway_tag: &str,
    ) -> Result<MixClient, SdkError> {
        let registry: Registry =
            serde_json::from_str(registry_json).map_err(|e| SdkError::Registry(e.to_string()))?;
        Ok(Self {
            registry,
            mean_delay_ms,
            reply_tag: tag_from_address(gateway_tag)?,
            pending: HashMap::new(),
            probes: HashSet::new(),
        })
    }

    /// Builds a request that asks for an answer over a return path the
    /// destination never sees.
    pub fn request(&mut self, destination: &str, body: &[u8]) -> Result<Outgoing, SdkError> {
        let path = self.path()?;
        let return_path = self
            .registry
            .select_return_path(&mut OsRng, self.mean_delay_ms, &path)
            .map_err(|e| SdkError::Registry(e.to_string()))?;
        let (surb, secret) =
            Surb::new(&return_path, self.reply_tag).map_err(|e| SdkError::Packet(e.to_string()))?;

        let id = secret.id;
        self.pending.insert(id, secret);
        let frame = Frame::Request(Request::new(body.to_vec(), Some(surb)));
        self.seal(&path, destination, &frame, Some(id.to_vec()))
    }

    /// Builds a request nothing can answer, for when nothing has to.
    pub fn send(&mut self, destination: &str, body: &[u8]) -> Result<Outgoing, SdkError> {
        let path = self.path()?;
        let frame = Frame::Request(Request::new(body.to_vec(), None));
        self.seal(&path, destination, &frame, None)
    }

    /// Builds a packet that carries nothing, so a packet that carries something
    /// is not distinguishable by the fact that it was sent.
    pub fn cover(&mut self, destination: &str) -> Result<Outgoing, SdkError> {
        let path = self.path()?;
        let frame = Frame::Request(Request::cover());
        self.seal(&path, destination, &frame, None)
    }

    /// Builds a packet addressed back to this client, to check the path it was
    /// routed over still carries traffic.
    pub fn probe(&mut self) -> Result<Outgoing, SdkError> {
        let path = self.path()?;
        let mut id = [0u8; 32];
        OsRng.fill_bytes(&mut id);
        self.probes.insert(id);

        let destination = address_from_tag(&self.reply_tag);
        let frame = Frame::Probe { id };
        self.seal(&path, &destination, &frame, Some(id.to_vec()))
    }

    /// Opens a frame the gateway delivered. Fails for anything this client is
    /// not waiting for, which is the only reason it can be handed bytes from a
    /// party it does not trust.
    pub fn accept(&mut self, delivered: &[u8]) -> Result<Delivery, SdkError> {
        match Frame::from_bytes(delivered).map_err(|e| SdkError::Frame(e.to_string()))? {
            Frame::Reply(Reply { surb_id, sealed }) => {
                let secret = self
                    .pending
                    .remove(&surb_id)
                    .ok_or(SdkError::UnknownReply)?;
                let body = secret
                    .open(&sealed)
                    .map_err(|e| SdkError::Packet(e.to_string()))?;
                Ok(Delivery {
                    id: surb_id.to_vec(),
                    body: Some(body),
                })
            }
            Frame::Probe { id } => {
                if !self.probes.remove(&id) {
                    return Err(SdkError::UnknownReply);
                }
                Ok(Delivery {
                    id: id.to_vec(),
                    body: None,
                })
            }
            Frame::Request(_) => Err(SdkError::RequestToClient),
        }
    }

    /// Requests still waiting for an answer.
    #[wasm_bindgen(getter)]
    pub fn in_flight(&self) -> usize {
        self.pending.len() + self.probes.len()
    }

    /// Gives up on a request, so a destination that never answers does not cost
    /// this client a reply-block secret for the rest of the session.
    pub fn forget(&mut self, id: &[u8]) {
        if let Ok(id) = to_id(id) {
            self.pending.remove(&id);
            self.probes.remove(&id);
        }
    }
}

impl MixClient {
    fn path(&self) -> Result<Vec<PathHop>, SdkError> {
        self.registry
            .select_path(&mut OsRng, self.mean_delay_ms)
            .map_err(|e| SdkError::Registry(e.to_string()))
    }

    fn seal(
        &self,
        path: &[PathHop],
        destination: &str,
        frame: &Frame,
        id: Option<Vec<u8>>,
    ) -> Result<Outgoing, SdkError> {
        let packet = Packet::build(&frame.to_bytes(), path, tag_from_address(destination)?)
            .map_err(|e| SdkError::Packet(e.to_string()))?;
        Ok(Outgoing {
            first_hop: path[0].id.to_vec(),
            packet: packet.to_bytes(),
            id,
        })
    }
}

/// Delivery tags are a 32-byte field holding a zero-padded `host:port`.
fn tag_from_address(address: &str) -> Result<[u8; 32], SdkError> {
    let bytes = address.as_bytes();
    if bytes.len() > 32 {
        return Err(SdkError::GatewayTag(address.to_string()));
    }
    let mut tag = [0u8; 32];
    tag[..bytes.len()].copy_from_slice(bytes);
    Ok(tag)
}

fn address_from_tag(tag: &[u8; 32]) -> String {
    let end = tag.iter().position(|b| *b == 0).unwrap_or(tag.len());
    String::from_utf8_lossy(&tag[..end]).into_owned()
}

fn to_id(bytes: &[u8]) -> Result<[u8; 32], SdkError> {
    bytes
        .try_into()
        .map_err(|_| SdkError::Frame("an id is 32 bytes".into()))
}

/// Parses a hex node id, for callers holding one as a string.
#[wasm_bindgen]
pub fn node_id(hex: &str) -> Result<Vec<u8>, SdkError> {
    decode_id(hex)
        .map(|id| id.to_vec())
        .map_err(|e| SdkError::Registry(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use erebus_sphinx::{PrivateKey, Processed};
    use erebus_topology::{encode_id, NodeRecord};

    fn network() -> (Registry, Vec<PrivateKey>) {
        let keys: Vec<PrivateKey> = (0..3).map(|_| PrivateKey::random()).collect();
        let registry = Registry {
            epoch_seed: "epoch-1".into(),
            nodes: keys
                .iter()
                .enumerate()
                .map(|(i, key)| NodeRecord {
                    id: encode_id(&key.public().to_bytes()),
                    address: format!("127.0.0.1:{}", 9000 + i),
                    stake: 1,
                })
                .collect(),
        };
        (registry, keys)
    }

    fn client() -> (MixClient, Vec<PrivateKey>) {
        let (registry, keys) = network();
        let json = serde_json::to_string(&registry).unwrap();
        (MixClient::new(&json, 0.0, "127.0.0.1:9200").unwrap(), keys)
    }

    /// Routes a packet the way three mix nodes would, and returns what the exit
    /// would have delivered.
    fn route(packet: &[u8], keys: &[PrivateKey]) -> Vec<u8> {
        let mut packet = Packet::from_bytes(packet).unwrap();
        loop {
            let peeled = keys
                .iter()
                .find_map(|key| key.process(&packet).ok())
                .expect("a hop that can peel this packet");
            match peeled {
                Processed::Forward { packet: next, .. } => packet = next,
                Processed::Deliver { message, .. } => return message,
            }
        }
    }

    #[test]
    fn a_request_survives_three_hops_and_the_reply_comes_back() {
        let (mut client, keys) = client();
        let out = client
            .request("127.0.0.1:9100", b"eth_blockNumber")
            .unwrap();
        assert_eq!(out.packet().len(), erebus_sphinx::PACKET_SIZE);
        assert_eq!(client.in_flight(), 1);

        let delivered = route(&out.packet(), &keys);
        let Frame::Request(request) = Frame::from_bytes(&delivered).unwrap() else {
            panic!("the exit should deliver a request");
        };
        assert_eq!(request.body, b"eth_blockNumber");

        // The service answers into the reply block, which routes back to the
        // gateway, which hands the frame to the client.
        let surb = request.surb.unwrap();
        let sealed = surb.seal_reply(b"0x1").unwrap();
        let reply = Frame::Reply(Reply {
            surb_id: surb.id,
            sealed,
        });
        let back = surb.into_packet(&reply.to_bytes()).unwrap();
        let delivered = route(&back.to_bytes(), &keys);

        let answer = client.accept(&delivered).unwrap();
        assert_eq!(answer.body().unwrap(), b"0x1");
        assert_eq!(client.in_flight(), 0);
    }

    #[test]
    fn a_probe_returns_to_the_client_that_sent_it() {
        let (mut client, keys) = client();
        let out = client.probe().unwrap();
        let delivered = route(&out.packet(), &keys);
        let returned = client.accept(&delivered).unwrap();
        assert!(returned.is_probe());
        assert_eq!(returned.id(), out.id().unwrap());
        assert_eq!(client.in_flight(), 0);
    }

    #[test]
    fn cover_traffic_is_the_same_size_as_everything_else() {
        let (mut client, keys) = client();
        let real = client.request("127.0.0.1:9100", b"eth_call").unwrap();
        let cover = client.cover("127.0.0.1:9100").unwrap();
        assert_eq!(real.packet().len(), cover.packet().len());
        assert!(cover.id().is_none());

        let delivered = route(&cover.packet(), &keys);
        let Frame::Request(request) = Frame::from_bytes(&delivered).unwrap() else {
            panic!("cover traffic is a request like any other");
        };
        assert!(request.cover);
    }

    #[test]
    fn a_reply_this_client_never_asked_for_is_refused() {
        let (mut client, _) = client();
        let frame = Frame::Reply(Reply {
            surb_id: [7u8; 32],
            sealed: vec![0; 48],
        });
        assert!(matches!(
            client.accept(&frame.to_bytes()),
            Err(SdkError::UnknownReply)
        ));
    }

    #[test]
    fn giving_up_on_a_request_drops_its_reply_block_secret() {
        let (mut client, _) = client();
        let out = client.request("127.0.0.1:9100", b"eth_call").unwrap();
        client.forget(&out.id().unwrap());
        assert_eq!(client.in_flight(), 0);
    }

    #[test]
    fn a_gateway_address_that_does_not_fit_a_tag_is_rejected() {
        let (registry, _) = network();
        let json = serde_json::to_string(&registry).unwrap();
        assert!(MixClient::new(&json, 0.0, &"a".repeat(33)).is_err());
        assert!(MixClient::new("not json", 0.0, "127.0.0.1:9200").is_err());
    }
}
