//! Single-use reply blocks.
//!
//! A client that wants an answer ships a pre-built return header. The responder
//! treats it as an opaque routing token: it can send exactly one reply along
//! that path, and it cannot read the destination, reuse the block, or tell it
//! apart from any other header.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use rand::{rngs::OsRng, RngCore};
use x25519_dalek::PublicKey;

use crate::crypto;
use crate::error::SphinxError;
use crate::header::{self, Header, PathHop};
use crate::payload;
use crate::Packet;

const NONCE: [u8; 12] = [0u8; 12];

/// The half of a reply block the client keeps: enough to recognise the reply
/// and open it, and nothing that would let anyone else do either.
#[derive(Debug, Clone)]
pub struct SurbSecret {
    pub id: [u8; 32],
    reply_key: [u8; 32],
}

impl SurbSecret {
    /// Opens a reply that arrived over the return path.
    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, SphinxError> {
        let cipher = ChaCha20Poly1305::new(&self.reply_key.into());
        cipher
            .decrypt(Nonce::from_slice(&NONCE), sealed)
            .map_err(|_| SphinxError::Aead)
    }
}

/// The half handed to the responder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surb {
    pub id: [u8; 32],
    pub first_hop: [u8; 32],
    header: Header,
    layer_keys: Vec<[u8; 32]>,
    exit_aead: [u8; 32],
    reply_key: [u8; 32],
}

impl Surb {
    /// Builds a reply block for `return_path`, whose exit delivers to
    /// `client_tag` — the address at which the client is listening.
    pub fn new(
        return_path: &[PathHop],
        client_tag: [u8; 32],
    ) -> Result<(Self, SurbSecret), SphinxError> {
        let public: Vec<PublicKey> = return_path.iter().map(|h| PublicKey::from(h.id)).collect();
        let path_keys = crypto::derive_path_keys(&public);
        let headers = header::build_headers(
            return_path,
            &path_keys.elements,
            &path_keys.keys,
            client_tag,
        )?;

        let mut id = [0u8; 32];
        OsRng.fill_bytes(&mut id);
        let mut reply_key = [0u8; 32];
        OsRng.fill_bytes(&mut reply_key);

        let exit_aead = path_keys.keys.last().expect("non-empty path").aead;
        let layer_keys = path_keys.keys.iter().map(|k| k.pi).collect();

        Ok((
            Self {
                id,
                first_hop: return_path[0].id,
                header: headers.into_iter().next().expect("non-empty path"),
                layer_keys,
                exit_aead,
                reply_key,
            },
            SurbSecret { id, reply_key },
        ))
    }

    /// Seals a reply under a key the return exit does not hold, so the last hop
    /// delivers ciphertext it cannot read.
    pub fn seal_reply(&self, reply: &[u8]) -> Result<Vec<u8>, SphinxError> {
        let cipher = ChaCha20Poly1305::new(&self.reply_key.into());
        cipher
            .encrypt(Nonce::from_slice(&NONCE), reply)
            .map_err(|_| SphinxError::Aead)
    }

    /// Wraps `message` for the return path. The caller decides what `message`
    /// is; it should carry [`Surb::id`] so the client can tell which reply block
    /// the reply belongs to.
    pub fn into_packet(self, message: &[u8]) -> Result<Packet, SphinxError> {
        let payload = payload::wrap_with(message, &self.layer_keys, &self.exit_aead)?;
        Ok(Packet {
            header: self.header,
            payload,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.id);
        out.extend_from_slice(&self.first_hop);
        out.extend_from_slice(&self.header.to_bytes());
        out.push(self.layer_keys.len() as u8);
        for key in &self.layer_keys {
            out.extend_from_slice(key);
        }
        out.extend_from_slice(&self.exit_aead);
        out.extend_from_slice(&self.reply_key);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SphinxError> {
        let fixed = 32 + 32 + Header::SIZE + 1;
        if bytes.len() < fixed {
            return Err(SphinxError::MalformedPacket);
        }
        let hops = bytes[fixed - 1] as usize;
        if hops == 0 || hops > header::MAX_HOPS || bytes.len() != fixed + hops * 32 + 64 {
            return Err(SphinxError::MalformedPacket);
        }

        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes[..32]);
        let mut first_hop = [0u8; 32];
        first_hop.copy_from_slice(&bytes[32..64]);
        let header = Header::from_bytes(&bytes[64..64 + Header::SIZE])?;

        let mut cursor = fixed;
        let mut layer_keys = Vec::with_capacity(hops);
        for _ in 0..hops {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes[cursor..cursor + 32]);
            layer_keys.push(key);
            cursor += 32;
        }
        let mut exit_aead = [0u8; 32];
        exit_aead.copy_from_slice(&bytes[cursor..cursor + 32]);
        let mut reply_key = [0u8; 32];
        reply_key.copy_from_slice(&bytes[cursor + 32..cursor + 64]);

        Ok(Self {
            id,
            first_hop,
            header,
            layer_keys,
            exit_aead,
            reply_key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PrivateKey, Processed};

    fn nodes(n: usize) -> Vec<PrivateKey> {
        (0..n).map(|_| PrivateKey::random()).collect()
    }

    fn path(nodes: &[PrivateKey]) -> Vec<PathHop> {
        nodes
            .iter()
            .map(|n| PathHop {
                id: n.public().to_bytes(),
                delay_ms: 0,
            })
            .collect()
    }

    #[test]
    fn a_reply_travels_the_return_path_and_only_the_client_can_read_it() {
        let return_nodes = nodes(3);
        let (surb, secret) = Surb::new(&path(&return_nodes), [3u8; 32]).unwrap();

        let sealed = surb.seal_reply(b"balance: 12 shares").unwrap();
        let mut packet = surb.into_packet(&sealed).unwrap();
        for node in &return_nodes[..2] {
            packet = match node.process(&packet).unwrap() {
                Processed::Forward { packet, .. } => packet,
                other => panic!("unexpected {other:?}"),
            };
        }

        match return_nodes[2].process(&packet).unwrap() {
            Processed::Deliver { tag, message, .. } => {
                assert_eq!(tag, [3u8; 32]);
                // The return exit delivers ciphertext, not the reply.
                assert_ne!(message.as_slice(), b"balance: 12 shares");
                assert_eq!(secret.open(&message).unwrap(), b"balance: 12 shares");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn wire_encoding_round_trips() {
        let return_nodes = nodes(3);
        let (surb, _) = Surb::new(&path(&return_nodes), [0u8; 32]).unwrap();
        assert_eq!(Surb::from_bytes(&surb.to_bytes()).unwrap(), surb);
    }

    #[test]
    fn a_truncated_reply_block_is_rejected() {
        let return_nodes = nodes(3);
        let (surb, _) = Surb::new(&path(&return_nodes), [0u8; 32]).unwrap();
        let bytes = surb.to_bytes();
        assert!(Surb::from_bytes(&bytes[..bytes.len() - 1]).is_err());
    }
}
