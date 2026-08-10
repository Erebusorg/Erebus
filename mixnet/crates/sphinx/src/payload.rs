//! The payload: fixed length, one stream layer per hop, one end-to-end AEAD.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};

use crate::crypto::{self, HopKeys};
use crate::error::SphinxError;

/// Total payload bytes on the wire, chosen so a packet is exactly 32 KB.
pub const PAYLOAD_SIZE: usize = 32768 - crate::header::Header::SIZE;
/// Bytes available to the sender: the AEAD tag and the 4-byte length prefix
/// come out of the fixed body.
pub const MAX_MESSAGE: usize = PAYLOAD_SIZE - 16 - 4;

const TAG: usize = 16;
const NONCE: [u8; 12] = [0u8; 12];

/// Seals `message` for the exit, then wraps one stream layer per hop.
///
/// Layers are length-preserving, so the payload is the same size on every link.
/// Only the exit's AEAD authenticates the payload: intermediate hops
/// authenticate the header (`gamma`) but not the body, so a bit-flip in transit
/// is detected at the exit rather than at the hop that introduced it. This is
/// the classic Sphinx trade-off, and the reason the format is not safe to use
/// with a payload the exit does not verify.
pub fn wrap(message: &[u8], keys: &[HopKeys]) -> Result<Vec<u8>, SphinxError> {
    let exit = keys.last().ok_or(SphinxError::PathLength(0))?;
    let layers: Vec<[u8; 32]> = keys.iter().map(|k| k.pi).collect();
    wrap_with(message, &layers, &exit.aead)
}

/// Same as [`wrap`], for a caller that holds only the raw layer keys — the
/// responder using a reply block, which is given keys but never a path.
pub fn wrap_with(
    message: &[u8],
    layer_keys: &[[u8; 32]],
    exit_aead: &[u8; 32],
) -> Result<Vec<u8>, SphinxError> {
    if message.len() > MAX_MESSAGE {
        return Err(SphinxError::MessageTooLong {
            len: message.len(),
            max: MAX_MESSAGE,
        });
    }
    if layer_keys.is_empty() {
        return Err(SphinxError::PathLength(0));
    }

    let mut body = vec![0u8; PAYLOAD_SIZE - TAG];
    body[..4].copy_from_slice(&(message.len() as u32).to_le_bytes());
    body[4..4 + message.len()].copy_from_slice(message);

    let cipher = ChaCha20Poly1305::new(exit_aead.into());
    let mut payload = cipher
        .encrypt(Nonce::from_slice(&NONCE), body.as_ref())
        .map_err(|_| SphinxError::Aead)?;
    debug_assert_eq!(payload.len(), PAYLOAD_SIZE);

    for key in layer_keys.iter().rev() {
        crypto::xor_in_place(&mut payload, &crypto::stream(key, PAYLOAD_SIZE));
    }

    Ok(payload)
}

/// Removes this hop's stream layer. Length is unchanged.
pub fn peel(payload: &mut [u8], keys: &HopKeys) {
    crypto::xor_in_place(payload, &crypto::stream(&keys.pi, payload.len()));
}

/// Opens a payload that has had every stream layer removed.
pub fn open(payload: &[u8], keys: &HopKeys) -> Result<Vec<u8>, SphinxError> {
    let cipher = ChaCha20Poly1305::new(&keys.aead.into());
    let body = cipher
        .decrypt(Nonce::from_slice(&NONCE), payload)
        .map_err(|_| SphinxError::Aead)?;

    if body.len() < 4 {
        return Err(SphinxError::MalformedPacket);
    }
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&body[..4]);
    let len = u32::from_le_bytes(len_bytes) as usize;
    if len > body.len() - 4 {
        return Err(SphinxError::MalformedPacket);
    }
    Ok(body[4..4 + len].to_vec())
}
