//! The node set, read from the registry contract.
//!
//! Everything here is one `eth_call` against `snapshot()`: the active nodes,
//! their endpoints, and the epoch seed clients derive layers from. Nothing is
//! signed, nothing is written, and no account is needed — a client reading the
//! node set is not a participant in anything on chain.
//!
//! The reason to read it from a contract rather than a file is not convenience:
//! a node set everyone can check is a node set nobody can tailor per client. A
//! directory server that hands one client a different set of nodes than another
//! has partitioned the anonymity set without anyone noticing.

use std::str::FromStr;
use std::time::Duration;

use alloy_primitives::Address;
use alloy_sol_types::{sol, SolCall};
use erebus_topology::{NodeRecord, Registry};
use serde::Deserialize;
use thiserror::Error;

mod source;

pub use source::RegistrySource;

sol! {
    #[derive(Debug)]
    struct Node {
        bytes32 key;
        string endpoint;
        uint256 stake;
        address operator;
        uint64 withdrawableAt;
    }

    function snapshot() external view returns (uint256 epoch, bytes32 seed, Node[] nodes);
}

/// How long to wait on an RPC endpoint before giving up on it.
const RPC_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("rpc transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("`{0}` is not a contract address")]
    BadAddress(String),
    #[error("the rpc returned an error: {0}")]
    Rpc(String),
    #[error("the rpc returned `{0}`, which is not hex-encoded call output")]
    BadHex(String),
    #[error("the call output does not match NodeRegistry.snapshot(): {0}")]
    BadReturn(String),
    #[error("the registry holds no active nodes")]
    Empty,
}

/// Where the node set lives.
#[derive(Debug, Clone)]
pub struct ChainRegistry {
    rpc: String,
    contract: Address,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: Option<String>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    message: String,
}

impl ChainRegistry {
    pub fn new(rpc: impl Into<String>, contract: &str) -> Result<Self, ChainError> {
        Ok(Self {
            rpc: rpc.into(),
            contract: Address::from_str(contract)
                .map_err(|_| ChainError::BadAddress(contract.to_string()))?,
            http: reqwest::Client::builder()
                .timeout(RPC_TIMEOUT)
                .build()
                .map_err(ChainError::Transport)?,
        })
    }

    /// Reads the current node set and epoch seed.
    ///
    /// The result is exactly what a JSON registry file would have held, so
    /// everything downstream — layer assignment, path selection, delays — is the
    /// same code whether the set came from a file or a contract.
    pub async fn fetch(&self) -> Result<Registry, ChainError> {
        registry_from(&self.call(&snapshotCall {}.abi_encode()).await?)
    }

    async fn call(&self, data: &[u8]) -> Result<Vec<u8>, ChainError> {
        let response: RpcResponse = self
            .http
            .post(&self.rpc)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "eth_call",
                "params": [
                    { "to": self.contract.to_string(), "data": format!("0x{}", hex(data)) },
                    "latest",
                ],
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(error) = response.error {
            return Err(ChainError::Rpc(error.message));
        }
        let result = response
            .result
            .ok_or_else(|| ChainError::Rpc("no result and no error".into()))?;
        decode_hex(result.trim_start_matches("0x")).ok_or(ChainError::BadHex(result))
    }
}

/// Turns `snapshot()` output into the registry the rest of the stack reads.
fn registry_from(output: &[u8]) -> Result<Registry, ChainError> {
    let snapshot = snapshotCall::abi_decode_returns(output)
        .map_err(|err| ChainError::BadReturn(err.to_string()))?;

    let nodes: Vec<NodeRecord> = snapshot
        .nodes
        .iter()
        .map(|node| NodeRecord {
            id: hex(node.key.as_slice()),
            address: node.endpoint.clone(),
            stake: u128::try_from(node.stake).unwrap_or(u128::MAX),
            // The operator is where a shielded fee spend sends this node's
            // share. It is already public in the registry, so carrying it costs
            // nothing: what a payout hides is the payer, not the payee.
            payout: (!node.operator.is_zero()).then(|| node.operator.to_string()),
        })
        .collect();

    if nodes.is_empty() {
        return Err(ChainError::Empty);
    }

    Ok(Registry {
        // The epoch number is part of the seed so that two epochs cannot share a
        // layer assignment even if the chain repeats a block hash.
        epoch_seed: format!("{}:{}", snapshot.epoch, hex(&snapshot.seed[..])),
        nodes,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{FixedBytes, U256};
    use alloy_sol_types::SolValue;

    use super::*;

    fn node(key: u8, endpoint: &str, stake: u128) -> Node {
        Node {
            key: FixedBytes::from([key; 32]),
            endpoint: endpoint.to_string(),
            stake: U256::from(stake),
            operator: Address::from([key; 20]),
            withdrawableAt: 0,
        }
    }

    fn encoded(epoch: u64, seed: [u8; 32], nodes: Vec<Node>) -> Vec<u8> {
        (U256::from(epoch), FixedBytes::from(seed), nodes).abi_encode_params()
    }

    fn decoded(output: &[u8]) -> Registry {
        registry_from(output).expect("a registry")
    }

    #[test]
    fn a_snapshot_becomes_the_registry_the_rest_of_the_stack_already_reads() {
        let output = encoded(
            7,
            [0xab; 32],
            vec![
                node(1, "203.0.113.7:9000", 1_000),
                node(2, "203.0.113.8:9000", 2_000),
            ],
        );
        let registry = decoded(&output);

        assert_eq!(registry.nodes.len(), 2);
        assert_eq!(registry.nodes[0].id, hex(&[1u8; 32]));
        assert_eq!(registry.nodes[0].address, "203.0.113.7:9000");
        assert_eq!(registry.nodes[1].stake, 2_000);
        // The node ids are the ids the Sphinx layer uses, unchanged.
        assert_eq!(registry.nodes[0].id_bytes().expect("an id"), [1u8; 32]);
        assert_eq!(
            registry.nodes[0].payout.as_deref(),
            Some(Address::from([1u8; 20]).to_string().as_str()),
            "the operator carries through as the fee payout address"
        );
    }

    #[test]
    fn a_node_with_no_operator_has_no_payout_address() {
        let mut zeroed = node(1, "203.0.113.7:9000", 1_000);
        zeroed.operator = Address::ZERO;
        let registry = decoded(&encoded(1, [0; 32], vec![zeroed]));
        assert!(registry.nodes[0].payout.is_none());
    }

    /// Two epochs must not derive the same layers, even from the same seed
    /// bytes: the epoch number is part of what clients hash.
    #[test]
    fn the_epoch_is_part_of_the_seed() {
        let first = decoded(&encoded(7, [0; 32], vec![node(1, "a:1", 1)]));
        let second = decoded(&encoded(8, [0; 32], vec![node(1, "a:1", 1)]));
        assert_ne!(first.epoch_seed, second.epoch_seed);
        assert!(first.epoch_seed.starts_with("7:"));
    }

    #[test]
    fn an_address_that_is_not_an_address_is_refused_before_any_request() {
        assert!(matches!(
            ChainRegistry::new("http://127.0.0.1:8545", "not-an-address"),
            Err(ChainError::BadAddress(_))
        ));
        assert!(ChainRegistry::new(
            "http://127.0.0.1:8545",
            "0x0000000000000000000000000000000000000001"
        )
        .is_ok());
    }

    #[test]
    fn a_registry_with_no_active_node_is_an_error_rather_than_an_empty_topology() {
        assert!(matches!(
            registry_from(&encoded(1, [0; 32], Vec::new())),
            Err(ChainError::Empty)
        ));
    }

    #[test]
    fn output_that_is_not_a_snapshot_is_refused() {
        assert!(matches!(
            registry_from(&[0u8; 7]),
            Err(ChainError::BadReturn(_))
        ));
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(decode_hex(&hex(&[0, 1, 0xff])), Some(vec![0, 1, 0xff]));
        assert_eq!(decode_hex("abc"), None);
        assert_eq!(decode_hex("zz"), None);
    }
}
