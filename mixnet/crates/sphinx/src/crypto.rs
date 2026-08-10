//! Key derivation and the stream/MAC primitives the packet format is built on.

use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::ChaCha20;
use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

pub const MAC_SIZE: usize = 16;

/// Every key a single hop needs, derived from that hop's shared secret.
#[derive(Clone)]
pub struct HopKeys {
    /// Header stream key: encrypts one layer of routing information.
    pub rho: [u8; 32],
    /// Header integrity key.
    pub mu: [u8; 32],
    /// Payload stream key: encrypts one length-preserving payload layer.
    pub pi: [u8; 32],
    /// Payload AEAD key. Only the final hop uses it.
    pub aead: [u8; 32],
}

impl HopKeys {
    pub fn derive(shared: &[u8; 32]) -> Self {
        Self {
            rho: kdf(shared, b"erebus/rho"),
            mu: kdf(shared, b"erebus/mu"),
            pi: kdf(shared, b"erebus/pi"),
            aead: kdf(shared, b"erebus/aead"),
        }
    }
}

pub fn kdf(shared: &[u8; 32], label: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(label);
    hasher.update(shared);
    hasher.finalize().into()
}

/// One ephemeral key exchange per hop.
///
/// A single blinded element chained across hops would be smaller, but an
/// independent element per hop gives the same unlinkability with no shared
/// derivation for a hop to get wrong: each hop learns only the element the
/// previous hop handed it, sealed inside that hop's own routing block.
pub struct PathKeys {
    pub elements: Vec<PublicKey>,
    pub keys: Vec<HopKeys>,
}

pub fn derive_path_keys(path: &[PublicKey]) -> PathKeys {
    let mut elements = Vec::with_capacity(path.len());
    let mut keys = Vec::with_capacity(path.len());

    for hop in path {
        let ephemeral = StaticSecret::random_from_rng(OsRng);
        let shared: [u8; 32] = ephemeral.diffie_hellman(hop).to_bytes();
        elements.push(PublicKey::from(&ephemeral));
        keys.push(HopKeys::derive(&shared));
    }

    PathKeys { elements, keys }
}

/// Recomputes a hop's keys from the element that arrived with the packet.
pub fn hop_keys(secret: &StaticSecret, element: &PublicKey) -> HopKeys {
    let shared: [u8; 32] = secret.diffie_hellman(element).to_bytes();
    HopKeys::derive(&shared)
}

/// Generates `len` bytes of keystream. Header and payload layers are both
/// length-preserving, so every layer is a stream XOR.
pub fn stream(key: &[u8; 32], len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    let mut cipher = ChaCha20::new(key.into(), &[0u8; 12].into());
    cipher.apply_keystream(&mut buf);
    buf
}

pub fn xor_in_place(target: &mut [u8], stream: &[u8]) {
    for (t, s) in target.iter_mut().zip(stream.iter()) {
        *t ^= s;
    }
}

pub fn mac(key: &[u8; 32], data: &[u8]) -> [u8; MAC_SIZE] {
    let mut hmac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("hmac accepts any key length");
    hmac.update(data);
    let full = hmac.finalize().into_bytes();
    let mut out = [0u8; MAC_SIZE];
    out.copy_from_slice(&full[..MAC_SIZE]);
    out
}

/// Tag used for replay detection: a hop that sees the same element twice is
/// seeing the same packet twice.
pub fn replay_tag(element: &PublicKey) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"erebus/replay");
    hasher.update(element.as_bytes());
    hasher.finalize().into()
}
