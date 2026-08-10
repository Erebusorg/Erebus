//! The packet header: a group element per hop, a constant-length encrypted
//! routing block chain, and an integrity MAC.

use rand::{rngs::OsRng, RngCore};
use x25519_dalek::PublicKey;

use crate::crypto::{self, HopKeys, MAC_SIZE};
use crate::error::SphinxError;

pub const MAX_HOPS: usize = 3;

/// Bytes of a single decrypted routing block.
///
/// ```text
/// 0        kind          1 = forward, 2 = deliver
/// 1..5     delay_ms      u32 LE, the hop's commanded Poisson delay
/// 5..37    next_element  X25519 element for the next hop (zero when delivering)
/// 37..69   next_id       next hop's public key, or the delivery tag
/// 69..96   reserved      zero
/// ```
pub const ROUTING_BLOCK: usize = 96;
const BLOCK_TOTAL: usize = ROUTING_BLOCK + MAC_SIZE;
pub const BETA_SIZE: usize = BLOCK_TOTAL * MAX_HOPS;

const KIND_FORWARD: u8 = 1;
const KIND_DELIVER: u8 = 2;

/// What a hop is told to do with a packet after it strips its layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Forward to `next_id`, handing it `next_element` and `next_beta`.
    Forward { next_id: [u8; 32], delay_ms: u32 },
    /// This hop is the exit: hand the payload to `tag`.
    Deliver { tag: [u8; 32], delay_ms: u32 },
}

impl Action {
    pub fn delay_ms(&self) -> u32 {
        match self {
            Action::Forward { delay_ms, .. } | Action::Deliver { delay_ms, .. } => *delay_ms,
        }
    }
}

/// A hop of a path as the sender describes it.
#[derive(Debug, Clone)]
pub struct PathHop {
    pub id: [u8; 32],
    pub delay_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub element: PublicKey,
    pub beta: Vec<u8>,
    pub mac: [u8; MAC_SIZE],
}

impl Header {
    pub const SIZE: usize = 32 + BETA_SIZE + MAC_SIZE;

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::SIZE);
        out.extend_from_slice(self.element.as_bytes());
        out.extend_from_slice(&self.beta);
        out.extend_from_slice(&self.mac);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SphinxError> {
        if bytes.len() != Self::SIZE {
            return Err(SphinxError::MalformedPacket);
        }
        let mut element = [0u8; 32];
        element.copy_from_slice(&bytes[..32]);
        let beta = bytes[32..32 + BETA_SIZE].to_vec();
        let mut mac = [0u8; MAC_SIZE];
        mac.copy_from_slice(&bytes[32 + BETA_SIZE..]);
        Ok(Self {
            element: PublicKey::from(element),
            beta,
            mac,
        })
    }
}

/// The header for hop `i`, for every `i` on the path.
///
/// Built innermost-first. The filler makes every hop's `beta` the same length
/// without the sender having to know anything a hop could measure: after a hop
/// shifts its own block off the front, the tail it appends is exactly the
/// keystream a downstream hop expects to find there.
pub fn build_headers(
    path: &[PathHop],
    elements: &[PublicKey],
    keys: &[HopKeys],
    delivery_tag: [u8; 32],
) -> Result<Vec<Header>, SphinxError> {
    let n = path.len();
    if n == 0 || n > MAX_HOPS {
        return Err(SphinxError::PathLength(n));
    }
    if elements.len() != n || keys.len() != n {
        return Err(SphinxError::PathLength(n));
    }

    let filler = build_filler(keys);

    // Innermost layer: the exit's block, random padding, then the filler.
    let mut block = [0u8; ROUTING_BLOCK];
    block[0] = KIND_DELIVER;
    block[1..5].copy_from_slice(&path[n - 1].delay_ms.to_le_bytes());
    block[37..69].copy_from_slice(&delivery_tag);

    let body_len = BETA_SIZE - filler.len();
    let mut beta = vec![0u8; body_len];
    beta[..ROUTING_BLOCK].copy_from_slice(&block);
    OsRng.fill_bytes(&mut beta[ROUTING_BLOCK..]);
    crypto::xor_in_place(&mut beta, &crypto::stream(&keys[n - 1].rho, body_len));
    beta.extend_from_slice(&filler);

    let mut headers = vec![Header {
        element: elements[n - 1],
        mac: crypto::mac(&keys[n - 1].mu, &beta),
        beta,
    }];

    // Each outer layer prepends its own block plus the MAC of the layer it wraps.
    for i in (0..n - 1).rev() {
        let inner = headers.last().expect("at least one layer");

        let mut block = [0u8; ROUTING_BLOCK];
        block[0] = KIND_FORWARD;
        block[1..5].copy_from_slice(&path[i].delay_ms.to_le_bytes());
        block[5..37].copy_from_slice(elements[i + 1].as_bytes());
        block[37..69].copy_from_slice(&path[i + 1].id);

        let mut beta = Vec::with_capacity(BETA_SIZE);
        beta.extend_from_slice(&block);
        beta.extend_from_slice(&inner.mac);
        beta.extend_from_slice(&inner.beta[..BETA_SIZE - BLOCK_TOTAL]);
        crypto::xor_in_place(&mut beta, &crypto::stream(&keys[i].rho, BETA_SIZE));

        headers.push(Header {
            element: elements[i],
            mac: crypto::mac(&keys[i].mu, &beta),
            beta,
        });
    }

    headers.reverse();
    Ok(headers)
}

/// Strips one layer. Returns the action for this hop and the header to forward.
pub fn peel(header: &Header, keys: &HopKeys) -> Result<(Action, Header), SphinxError> {
    if header.beta.len() != BETA_SIZE {
        return Err(SphinxError::MalformedPacket);
    }
    if crypto::mac(&keys.mu, &header.beta) != header.mac {
        return Err(SphinxError::IntegrityFailure);
    }

    let mut extended = header.beta.clone();
    extended.extend_from_slice(&[0u8; BLOCK_TOTAL]);
    crypto::xor_in_place(
        &mut extended,
        &crypto::stream(&keys.rho, BETA_SIZE + BLOCK_TOTAL),
    );

    let block = &extended[..ROUTING_BLOCK];
    let mut next_mac = [0u8; MAC_SIZE];
    next_mac.copy_from_slice(&extended[ROUTING_BLOCK..BLOCK_TOTAL]);
    let next_beta = extended[BLOCK_TOTAL..].to_vec();

    let mut delay_bytes = [0u8; 4];
    delay_bytes.copy_from_slice(&block[1..5]);
    let delay_ms = u32::from_le_bytes(delay_bytes);

    let mut next_element = [0u8; 32];
    next_element.copy_from_slice(&block[5..37]);
    let mut next_id = [0u8; 32];
    next_id.copy_from_slice(&block[37..69]);

    let action = match block[0] {
        KIND_FORWARD => Action::Forward { next_id, delay_ms },
        KIND_DELIVER => Action::Deliver {
            tag: next_id,
            delay_ms,
        },
        _ => return Err(SphinxError::IntegrityFailure),
    };

    Ok((
        action,
        Header {
            element: PublicKey::from(next_element),
            beta: next_beta,
            mac: next_mac,
        },
    ))
}

/// Pseudorandom tail shared by sender and hops, `(n - 1)` blocks long.
fn build_filler(keys: &[HopKeys]) -> Vec<u8> {
    let mut filler: Vec<u8> = Vec::new();
    for key in keys.iter().take(keys.len().saturating_sub(1)) {
        filler.extend_from_slice(&[0u8; BLOCK_TOTAL]);
        let stream = crypto::stream(&key.rho, BETA_SIZE + BLOCK_TOTAL);
        let offset = BETA_SIZE + BLOCK_TOTAL - filler.len();
        crypto::xor_in_place(&mut filler, &stream[offset..]);
    }
    filler
}
