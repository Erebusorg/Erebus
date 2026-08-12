//! The node set, how it is partitioned into layers, and how a client picks a
//! path through it.
//!
//! On testnet the registry is a JSON file. On chain it is a contract, and only
//! [`Registry::load`] changes: layer assignment and path selection are already
//! derived from public data alone, so every client reaches the same topology
//! with no directory server to trust and nothing to coordinate.

use std::collections::HashSet;
use std::path::Path;

use erebus_sphinx::PathHop;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const LAYERS: usize = 3;

#[derive(Debug, Error)]
pub enum TopologyError {
    #[error("registry file: {0}")]
    Io(#[from] std::io::Error),
    #[error("registry json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("node id `{0}` is not 32 bytes of hex")]
    BadNodeId(String),
    #[error("layer {layer} has no nodes")]
    EmptyLayer { layer: usize },
    #[error("no node registered with id {0}")]
    UnknownNode(String),
    #[error("node {0} has no payout address, so it cannot be paid")]
    NoPayout(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRecord {
    /// Hex-encoded X25519 public key. Also the node's identity.
    pub id: String,
    /// Where to reach it, `host:port`.
    pub address: String,
    /// Stake, in the smallest unit. Only meaningful once the registry is on chain.
    #[serde(default)]
    pub stake: u128,
    /// Where fees for this node are paid: the operator address from the
    /// registry. Absent for a registry file that predates shielded fees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout: Option<String>,
}

impl NodeRecord {
    pub fn id_bytes(&self) -> Result<[u8; 32], TopologyError> {
        decode_id(&self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    /// Epoch seed. On chain this is the hash of the epoch's first block.
    pub epoch_seed: String,
    pub nodes: Vec<NodeRecord>,
}

impl Registry {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, TopologyError> {
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), TopologyError> {
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn find(&self, id: &[u8; 32]) -> Option<&NodeRecord> {
        let hex = encode_id(id);
        self.nodes.iter().find(|n| n.id == hex)
    }

    pub fn address_of(&self, id: &[u8; 32]) -> Result<String, TopologyError> {
        self.find(id)
            .map(|n| n.address.clone())
            .ok_or_else(|| TopologyError::UnknownNode(encode_id(id)))
    }

    /// The payout addresses of the nodes on a path, in hop order.
    ///
    /// This is what a payer spends a fee note on. It names the three operators
    /// that carried a route, not the route: the amounts are equal and the
    /// addresses are public, so the payout says which nodes were used and
    /// nothing about by whom or for what.
    pub fn payees(&self, path: &[PathHop]) -> Result<Vec<String>, TopologyError> {
        path.iter()
            .map(|hop| {
                let node = self
                    .find(&hop.id)
                    .ok_or_else(|| TopologyError::UnknownNode(encode_id(&hop.id)))?;
                node.payout
                    .clone()
                    .ok_or_else(|| TopologyError::NoPayout(node.id.clone()))
            })
            .collect()
    }

    /// Partitions the node set into layers.
    ///
    /// Assignment is a deterministic function of the epoch seed and each node's
    /// public key, so an operator cannot choose to sit in the exit layer, where
    /// the most valuable metadata is, and cannot know next epoch's position
    /// early enough to prepare for it.
    pub fn layers(&self) -> Result<Vec<Vec<NodeRecord>>, TopologyError> {
        let mut ranked: Vec<([u8; 32], NodeRecord)> = self
            .nodes
            .iter()
            .map(|node| {
                let mut hasher = Sha256::new();
                hasher.update(b"erebus/layer");
                hasher.update(self.epoch_seed.as_bytes());
                hasher.update(node.id_bytes()?);
                Ok((hasher.finalize().into(), node.clone()))
            })
            .collect::<Result<_, TopologyError>>()?;

        ranked.sort_by_key(|(rank, _)| *rank);

        let mut layers = vec![Vec::new(); LAYERS];
        for (i, (_, node)) in ranked.into_iter().enumerate() {
            layers[i % LAYERS].push(node);
        }

        for (layer, nodes) in layers.iter().enumerate() {
            if nodes.is_empty() {
                return Err(TopologyError::EmptyLayer { layer });
            }
        }
        Ok(layers)
    }

    /// Picks one node per layer and assigns each an independent exponential
    /// delay with mean `mean_delay_ms`.
    ///
    /// The delay is chosen by the client, not the node: a node that departs from
    /// the delay it was handed is detectable by loop probes, and a client that
    /// wants a larger anonymity set can pay for it in latency without asking
    /// anyone's permission.
    pub fn select_path<R: Rng>(
        &self,
        rng: &mut R,
        mean_delay_ms: f64,
    ) -> Result<Vec<PathHop>, TopologyError> {
        let layers = self.layers()?;
        let mut path = Vec::with_capacity(LAYERS);

        for nodes in &layers {
            let node = nodes.choose(rng).expect("layer is non-empty");
            path.push(PathHop {
                id: node.id_bytes()?,
                delay_ms: exponential_delay(rng, mean_delay_ms),
            });
        }

        Ok(path)
    }

    /// A return path for a reply block: independent of the forward path, so a
    /// node that saw the request is unlikely to also see the reply.
    pub fn select_return_path<R: Rng>(
        &self,
        rng: &mut R,
        mean_delay_ms: f64,
        avoid: &[PathHop],
    ) -> Result<Vec<PathHop>, TopologyError> {
        let used: HashSet<[u8; 32]> = avoid.iter().map(|h| h.id).collect();
        let layers = self.layers()?;
        let mut path = Vec::with_capacity(LAYERS);

        for nodes in &layers {
            let free: Vec<&NodeRecord> = nodes
                .iter()
                .filter(|n| n.id_bytes().map(|id| !used.contains(&id)).unwrap_or(false))
                .collect();
            // Fall back to the whole layer when it has no unused node: a smaller
            // network should still work, with weaker separation.
            let node = if free.is_empty() {
                nodes.choose(rng).expect("layer is non-empty")
            } else {
                free.choose(rng).copied().expect("checked non-empty")
            };
            path.push(PathHop {
                id: node.id_bytes()?,
                delay_ms: exponential_delay(rng, mean_delay_ms),
            });
        }

        Ok(path)
    }
}

/// Exponential delay, rounded to milliseconds.
///
/// Memorylessness is the point: the residual delay of a queued packet is
/// independent of how long it has already waited, so the order in which a node
/// emits packets says nothing about the order they arrived.
pub fn exponential_delay<R: Rng>(rng: &mut R, mean_ms: f64) -> u32 {
    if mean_ms <= 0.0 {
        return 0;
    }
    // Inverse transform: -ln(U) for U uniform on (0, 1] is Exp(1).
    let u: f64 = rng.gen_range(f64::MIN_POSITIVE..=1.0);
    (-u.ln() * mean_ms).round().min(u32::MAX as f64) as u32
}

pub fn encode_id(id: &[u8; 32]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn decode_id(hex: &str) -> Result<[u8; 32], TopologyError> {
    if hex.len() != 64 {
        return Err(TopologyError::BadNodeId(hex.to_string()));
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| TopologyError::BadNodeId(hex.to_string()))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn registry(n: usize, seed: &str) -> Registry {
        Registry {
            epoch_seed: seed.to_string(),
            nodes: (0..n)
                .map(|i| NodeRecord {
                    id: encode_id(&[i as u8 + 1; 32]),
                    address: format!("127.0.0.1:{}", 9000 + i),
                    stake: 1,
                    payout: Some(format!("0x{:040x}", i + 1)),
                })
                .collect(),
        }
    }

    #[test]
    fn layer_assignment_is_deterministic_and_covers_every_node() {
        let reg = registry(9, "epoch-1");
        let a = reg.layers().unwrap();
        let b = reg.layers().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.iter().map(Vec::len).sum::<usize>(), 9);
        assert!(a.iter().all(|l| l.len() == 3));
    }

    #[test]
    fn a_new_epoch_reshuffles_the_layers() {
        let first = registry(9, "epoch-1").layers().unwrap();
        let second = registry(9, "epoch-2").layers().unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn a_layer_with_no_nodes_is_an_error() {
        let reg = registry(2, "epoch-1");
        assert!(matches!(
            reg.layers(),
            Err(TopologyError::EmptyLayer { layer: 2 })
        ));
    }

    #[test]
    fn a_path_takes_one_node_from_each_layer() {
        let reg = registry(9, "epoch-1");
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let path = reg.select_path(&mut rng, 30.0).unwrap();
        assert_eq!(path.len(), LAYERS);

        let layers = reg.layers().unwrap();
        for (hop, layer) in path.iter().zip(layers.iter()) {
            assert!(layer.iter().any(|n| n.id_bytes().unwrap() == hop.id));
        }
    }

    #[test]
    fn a_return_path_avoids_the_forward_path_when_it_can() {
        let reg = registry(9, "epoch-1");
        let mut rng = ChaCha8Rng::seed_from_u64(11);
        let forward = reg.select_path(&mut rng, 30.0).unwrap();
        let back = reg.select_return_path(&mut rng, 30.0, &forward).unwrap();

        let forward_ids: HashSet<[u8; 32]> = forward.iter().map(|h| h.id).collect();
        assert!(back.iter().all(|h| !forward_ids.contains(&h.id)));
    }

    #[test]
    fn delays_vary_and_average_near_the_requested_mean() {
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let samples: Vec<u32> = (0..4000)
            .map(|_| exponential_delay(&mut rng, 50.0))
            .collect();
        let mean = samples.iter().map(|d| *d as f64).sum::<f64>() / samples.len() as f64;

        assert!((mean - 50.0).abs() < 5.0, "mean was {mean}");
        assert!(samples.iter().collect::<HashSet<_>>().len() > 50);
        assert_eq!(exponential_delay(&mut rng, 0.0), 0);
    }

    #[test]
    fn a_path_names_the_operators_that_get_paid() {
        let reg = registry(9, "epoch-1");
        let mut rng = ChaCha8Rng::seed_from_u64(5);
        let path = reg.select_path(&mut rng, 30.0).unwrap();

        let payees = reg.payees(&path).unwrap();
        assert_eq!(payees.len(), LAYERS);
        assert!(payees.iter().all(|p| p.starts_with("0x")));
    }

    #[test]
    fn a_node_with_no_payout_address_cannot_be_paid() {
        let mut reg = registry(9, "epoch-1");
        for node in &mut reg.nodes {
            node.payout = None;
        }
        let mut rng = ChaCha8Rng::seed_from_u64(5);
        let path = reg.select_path(&mut rng, 30.0).unwrap();

        assert!(matches!(reg.payees(&path), Err(TopologyError::NoPayout(_))));
    }

    #[test]
    fn ids_round_trip_through_hex() {
        let id = [0xab; 32];
        assert_eq!(decode_id(&encode_id(&id)).unwrap(), id);
        assert!(decode_id("nope").is_err());
    }
}
