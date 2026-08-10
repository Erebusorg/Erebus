//! Sphinx packets for the Erebus mixnet.
//!
//! A packet is exactly [`PACKET_SIZE`] bytes on every link, and no field
//! survives a hop: the group element is per-hop, the routing chain is
//! re-encrypted, and the payload is re-randomised. A hop learns the previous
//! hop, the next hop, and how long to hold the packet. Nothing else.
//!
//! ```
//! use erebus_sphinx::{Packet, PathHop, PrivateKey};
//!
//! let nodes: Vec<PrivateKey> = (0..3).map(|_| PrivateKey::random()).collect();
//! let path: Vec<PathHop> = nodes
//!     .iter()
//!     .map(|n| PathHop { id: n.public().to_bytes(), delay_ms: 0 })
//!     .collect();
//!
//! let mut packet = Packet::build(b"buy 10 AAPL", &path, *b"sink____________________________").unwrap();
//! for node in &nodes[..2] {
//!     packet = match node.process(&packet).unwrap() {
//!         erebus_sphinx::Processed::Forward { packet, .. } => packet,
//!         _ => panic!("expected a forward"),
//!     };
//! }
//! match nodes[2].process(&packet).unwrap() {
//!     erebus_sphinx::Processed::Deliver { message, .. } => assert_eq!(message, b"buy 10 AAPL"),
//!     _ => panic!("expected a delivery"),
//! }
//! ```

pub mod crypto;
pub mod error;
pub mod header;
pub mod payload;
pub mod surb;

use rand::rngs::OsRng;
use x25519_dalek::{PublicKey, StaticSecret};

pub use error::SphinxError;
pub use header::{Action, Header, PathHop, MAX_HOPS};
pub use payload::{MAX_MESSAGE, PAYLOAD_SIZE};
pub use surb::{Surb, SurbSecret};

/// Every packet is this size on every link, in both directions.
pub const PACKET_SIZE: usize = 32768;

/// A mix node's long-term key.
pub struct PrivateKey(StaticSecret);

impl PrivateKey {
    pub fn random() -> Self {
        Self(StaticSecret::random_from_rng(OsRng))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(StaticSecret::from(bytes))
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn public(&self) -> PublicKey {
        PublicKey::from(&self.0)
    }

    /// Strips this hop's layer from a packet.
    pub fn process(&self, packet: &Packet) -> Result<Processed, SphinxError> {
        let keys = crypto::hop_keys(&self.0, &packet.header.element);
        let (action, next_header) = header::peel(&packet.header, &keys)?;

        let mut payload = packet.payload.clone();
        payload::peel(&mut payload, &keys);

        match action {
            Action::Forward { next_id, delay_ms } => Ok(Processed::Forward {
                next_id,
                delay_ms,
                packet: Packet {
                    header: next_header,
                    payload,
                },
            }),
            Action::Deliver { tag, delay_ms } => Ok(Processed::Deliver {
                tag,
                delay_ms,
                message: payload::open(&payload, &keys)?,
            }),
        }
    }

    /// Replay tag for a packet, so a node can refuse to process one twice.
    pub fn replay_tag(packet: &Packet) -> [u8; 32] {
        crypto::replay_tag(&packet.header.element)
    }
}

/// The result of a node processing a packet.
#[derive(Debug, Clone)]
pub enum Processed {
    Forward {
        next_id: [u8; 32],
        delay_ms: u32,
        packet: Packet,
    },
    Deliver {
        tag: [u8; 32],
        delay_ms: u32,
        message: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub header: Header,
    pub payload: Vec<u8>,
}

impl Packet {
    /// Builds a packet for `path`, to be delivered at the exit to `tag`.
    pub fn build(message: &[u8], path: &[PathHop], tag: [u8; 32]) -> Result<Self, SphinxError> {
        let public: Vec<PublicKey> = path.iter().map(|h| PublicKey::from(h.id)).collect();
        let path_keys = crypto::derive_path_keys(&public);
        let headers = header::build_headers(path, &path_keys.elements, &path_keys.keys, tag)?;
        let payload = payload::wrap(message, &path_keys.keys)?;

        Ok(Self {
            header: headers.into_iter().next().expect("non-empty path"),
            payload,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.header.to_bytes();
        out.extend_from_slice(&self.payload);
        debug_assert_eq!(out.len(), PACKET_SIZE);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SphinxError> {
        if bytes.len() != PACKET_SIZE {
            return Err(SphinxError::MalformedPacket);
        }
        Ok(Self {
            header: Header::from_bytes(&bytes[..Header::SIZE])?,
            payload: bytes[Header::SIZE..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes(n: usize) -> Vec<PrivateKey> {
        (0..n).map(|_| PrivateKey::random()).collect()
    }

    fn path(nodes: &[PrivateKey], delay_ms: u32) -> Vec<PathHop> {
        nodes
            .iter()
            .map(|n| PathHop {
                id: n.public().to_bytes(),
                delay_ms,
            })
            .collect()
    }

    fn run(nodes: &[PrivateKey], packet: Packet) -> Processed {
        let mut current = packet;
        for node in &nodes[..nodes.len() - 1] {
            current = match node.process(&current).unwrap() {
                Processed::Forward { packet, .. } => packet,
                other => panic!("unexpected {other:?}"),
            };
        }
        nodes.last().unwrap().process(&current).unwrap()
    }

    #[test]
    fn three_hop_round_trip() {
        let nodes = nodes(3);
        let packet = Packet::build(b"hello", &path(&nodes, 25), [7u8; 32]).unwrap();

        match run(&nodes, packet) {
            Processed::Deliver {
                tag,
                delay_ms,
                message,
            } => {
                assert_eq!(message, b"hello");
                assert_eq!(tag, [7u8; 32]);
                assert_eq!(delay_ms, 25);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn shorter_paths_work() {
        for hops in 1..=MAX_HOPS {
            let nodes = nodes(hops);
            let packet = Packet::build(b"x", &path(&nodes, 0), [1u8; 32]).unwrap();
            match run(&nodes, packet) {
                Processed::Deliver { message, .. } => assert_eq!(message, b"x"),
                other => panic!("{hops} hops: unexpected {other:?}"),
            }
        }
    }

    #[test]
    fn every_hop_sees_a_constant_size_packet() {
        let nodes = nodes(3);
        let mut packet = Packet::build(b"hi", &path(&nodes, 0), [0u8; 32]).unwrap();
        assert_eq!(packet.to_bytes().len(), PACKET_SIZE);

        for node in &nodes[..2] {
            packet = match node.process(&packet).unwrap() {
                Processed::Forward { packet, .. } => packet,
                other => panic!("unexpected {other:?}"),
            };
            assert_eq!(packet.to_bytes().len(), PACKET_SIZE);
        }
    }

    #[test]
    fn no_field_is_shared_between_hops() {
        let nodes = nodes(3);
        let first = Packet::build(b"hi", &path(&nodes, 0), [0u8; 32]).unwrap();
        let second = match nodes[0].process(&first).unwrap() {
            Processed::Forward { packet, .. } => packet,
            other => panic!("unexpected {other:?}"),
        };

        assert_ne!(first.header.element, second.header.element);
        assert_ne!(first.header.beta, second.header.beta);
        assert_ne!(first.header.mac, second.header.mac);
        assert_ne!(first.payload, second.payload);
    }

    #[test]
    fn the_same_message_never_produces_the_same_packet() {
        let nodes = nodes(3);
        let hops = path(&nodes, 0);
        let a = Packet::build(b"same", &hops, [0u8; 32]).unwrap();
        let b = Packet::build(b"same", &hops, [0u8; 32]).unwrap();
        assert_ne!(a.to_bytes(), b.to_bytes());
    }

    #[test]
    fn a_hop_cannot_read_the_payload_it_forwards() {
        let nodes = nodes(3);
        let packet = Packet::build(b"buy 10 AAPL", &path(&nodes, 0), [0u8; 32]).unwrap();
        let forwarded = match nodes[0].process(&packet).unwrap() {
            Processed::Forward { packet, .. } => packet,
            other => panic!("unexpected {other:?}"),
        };
        assert!(!forwarded.payload.windows(11).any(|w| w == b"buy 10 AAPL"));
    }

    #[test]
    fn tampering_with_the_header_is_detected() {
        let nodes = nodes(3);
        let mut packet = Packet::build(b"hi", &path(&nodes, 0), [0u8; 32]).unwrap();
        packet.header.beta[0] ^= 0xff;

        assert!(matches!(
            nodes[0].process(&packet),
            Err(SphinxError::IntegrityFailure)
        ));
    }

    #[test]
    fn tampering_with_the_payload_is_detected_at_the_exit() {
        let nodes = nodes(3);
        let mut packet = Packet::build(b"hi", &path(&nodes, 0), [0u8; 32]).unwrap();
        packet.payload[100] ^= 0xff;

        let mut current = packet;
        for node in &nodes[..2] {
            current = match node.process(&current).unwrap() {
                Processed::Forward { packet, .. } => packet,
                other => panic!("unexpected {other:?}"),
            };
        }
        assert!(matches!(nodes[2].process(&current), Err(SphinxError::Aead)));
    }

    #[test]
    fn a_packet_is_not_processable_by_the_wrong_node() {
        let nodes = nodes(3);
        let stranger = PrivateKey::random();
        let packet = Packet::build(b"hi", &path(&nodes, 0), [0u8; 32]).unwrap();

        assert!(matches!(
            stranger.process(&packet),
            Err(SphinxError::IntegrityFailure)
        ));
    }

    #[test]
    fn replay_tags_identify_the_same_packet() {
        let nodes = nodes(3);
        let packet = Packet::build(b"hi", &path(&nodes, 0), [0u8; 32]).unwrap();
        let copy = Packet::from_bytes(&packet.to_bytes()).unwrap();
        let other = Packet::build(b"hi", &path(&nodes, 0), [0u8; 32]).unwrap();

        assert_eq!(
            PrivateKey::replay_tag(&packet),
            PrivateKey::replay_tag(&copy)
        );
        assert_ne!(
            PrivateKey::replay_tag(&packet),
            PrivateKey::replay_tag(&other)
        );
    }

    #[test]
    fn wire_encoding_round_trips() {
        let nodes = nodes(3);
        let packet = Packet::build(b"hi", &path(&nodes, 0), [0u8; 32]).unwrap();
        assert_eq!(Packet::from_bytes(&packet.to_bytes()).unwrap(), packet);
    }

    #[test]
    fn an_oversized_message_is_rejected() {
        let nodes = nodes(3);
        let too_long = vec![0u8; MAX_MESSAGE + 1];
        assert!(matches!(
            Packet::build(&too_long, &path(&nodes, 0), [0u8; 32]),
            Err(SphinxError::MessageTooLong { .. })
        ));
    }

    #[test]
    fn a_message_of_exactly_the_maximum_size_fits() {
        let nodes = nodes(3);
        let message = vec![9u8; MAX_MESSAGE];
        let packet = Packet::build(&message, &path(&nodes, 0), [0u8; 32]).unwrap();
        match run(&nodes, packet) {
            Processed::Deliver { message: got, .. } => assert_eq!(got, message),
            other => panic!("unexpected {other:?}"),
        }
    }
}
