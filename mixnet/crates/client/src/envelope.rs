//! What a client puts inside a packet, and what comes back.
//!
//! This layer sits above Sphinx and is the only part the exit's destination
//! service parses. It carries the request body and, optionally, the reply block
//! the service must use to answer — the service never learns an address to
//! answer to.

use anyhow::{bail, Result};
use erebus_sphinx::Surb;

const VERSION: u8 = 1;
const FLAG_SURB: u8 = 1 << 0;
/// Set on cover traffic, which a destination service is expected to discard.
const FLAG_COVER: u8 = 1 << 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub body: Vec<u8>,
    pub surb: Option<Surb>,
    pub cover: bool,
}

impl Request {
    pub fn new(body: Vec<u8>, surb: Option<Surb>) -> Self {
        Self {
            body,
            surb,
            cover: false,
        }
    }

    /// A request whose only purpose is to be indistinguishable from a real one.
    pub fn cover() -> Self {
        Self {
            body: Vec::new(),
            surb: None,
            cover: true,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut flags = 0u8;
        if self.surb.is_some() {
            flags |= FLAG_SURB;
        }
        if self.cover {
            flags |= FLAG_COVER;
        }

        let mut out = vec![VERSION, flags];
        if let Some(surb) = &self.surb {
            let bytes = surb.to_bytes();
            out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&bytes);
        }
        out.extend_from_slice(&(self.body.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.body);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 || bytes[0] != VERSION {
            bail!("unrecognised request envelope");
        }
        let flags = bytes[1];
        let mut cursor = 2;

        let surb = if flags & FLAG_SURB != 0 {
            let len = read_u16(bytes, &mut cursor)? as usize;
            let end = cursor + len;
            if end > bytes.len() {
                bail!("reply block runs past the end of the envelope");
            }
            let surb = Surb::from_bytes(&bytes[cursor..end])?;
            cursor = end;
            Some(surb)
        } else {
            None
        };

        let len = read_u32(bytes, &mut cursor)? as usize;
        if cursor + len > bytes.len() {
            bail!("body runs past the end of the envelope");
        }

        Ok(Self {
            body: bytes[cursor..cursor + len].to_vec(),
            surb,
            cover: flags & FLAG_COVER != 0,
        })
    }
}

/// A reply as it is delivered to the client: the reply block's id, so the client
/// knows which pending request it answers, and the sealed body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub surb_id: [u8; 32],
    pub sealed: Vec<u8>,
}

impl Reply {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.surb_id.to_vec();
        out.extend_from_slice(&self.sealed);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 32 {
            bail!("reply is too short to carry a reply block id");
        }
        let mut surb_id = [0u8; 32];
        surb_id.copy_from_slice(&bytes[..32]);
        Ok(Self {
            surb_id,
            sealed: bytes[32..].to_vec(),
        })
    }
}

/// Everything that leaves the mixnet, tagged so the receiver knows what it is
/// looking at without guessing from the first byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// A client's request, delivered to a destination service.
    Request(Request),
    /// A service's answer, delivered to a client.
    Reply(Reply),
    /// A packet a client sent to itself, to check the path it was routed over
    /// still carries traffic and still applies the delays it was handed.
    Probe { id: [u8; 32] },
}

impl Frame {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Frame::Request(request) => {
                let mut out = vec![1u8];
                out.extend_from_slice(&request.to_bytes());
                out
            }
            Frame::Reply(reply) => {
                let mut out = vec![2u8];
                out.extend_from_slice(&reply.to_bytes());
                out
            }
            Frame::Probe { id } => {
                let mut out = vec![3u8];
                out.extend_from_slice(id);
                out
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        match bytes.first() {
            Some(1) => Ok(Frame::Request(Request::from_bytes(&bytes[1..])?)),
            Some(2) => Ok(Frame::Reply(Reply::from_bytes(&bytes[1..])?)),
            Some(3) if bytes.len() == 33 => {
                let mut id = [0u8; 32];
                id.copy_from_slice(&bytes[1..]);
                Ok(Frame::Probe { id })
            }
            _ => bail!("unrecognised frame"),
        }
    }
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16> {
    if *cursor + 2 > bytes.len() {
        bail!("truncated envelope");
    }
    let value = u16::from_le_bytes([bytes[*cursor], bytes[*cursor + 1]]);
    *cursor += 2;
    Ok(value)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32> {
    if *cursor + 4 > bytes.len() {
        bail!("truncated envelope");
    }
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&bytes[*cursor..*cursor + 4]);
    *cursor += 4;
    Ok(u32::from_le_bytes(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use erebus_sphinx::{PathHop, PrivateKey};

    fn surb() -> Surb {
        let nodes: Vec<PrivateKey> = (0..3).map(|_| PrivateKey::random()).collect();
        let path: Vec<PathHop> = nodes
            .iter()
            .map(|n| PathHop {
                id: n.public().to_bytes(),
                delay_ms: 0,
            })
            .collect();
        Surb::new(&path, [1u8; 32]).unwrap().0
    }

    #[test]
    fn a_request_with_a_reply_block_round_trips() {
        let request = Request::new(b"eth_call".to_vec(), Some(surb()));
        assert_eq!(Request::from_bytes(&request.to_bytes()).unwrap(), request);
    }

    #[test]
    fn a_request_without_a_reply_block_round_trips() {
        let request = Request::new(b"fire and forget".to_vec(), None);
        let decoded = Request::from_bytes(&request.to_bytes()).unwrap();
        assert_eq!(decoded, request);
        assert!(decoded.surb.is_none());
    }

    #[test]
    fn cover_traffic_is_marked_but_otherwise_a_normal_request() {
        let decoded = Request::from_bytes(&Request::cover().to_bytes()).unwrap();
        assert!(decoded.cover);
        assert!(decoded.body.is_empty());
    }

    #[test]
    fn a_truncated_envelope_is_rejected() {
        let bytes = Request::new(b"body".to_vec(), Some(surb())).to_bytes();
        assert!(Request::from_bytes(&bytes[..bytes.len() - 2]).is_err());
        assert!(Request::from_bytes(&[]).is_err());
    }

    #[test]
    fn a_reply_round_trips() {
        let reply = Reply {
            surb_id: [9u8; 32],
            sealed: vec![1, 2, 3],
        };
        assert_eq!(Reply::from_bytes(&reply.to_bytes()).unwrap(), reply);
        assert!(Reply::from_bytes(&[0u8; 8]).is_err());
    }
}
