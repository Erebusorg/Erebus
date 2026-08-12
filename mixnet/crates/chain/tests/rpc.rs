//! The read path against a JSON-RPC endpoint, without a chain.
//!
//! `NodeRegistry.snapshot()` is exercised against a real deployment by
//! `scripts/chain-devnet.sh`; what is worth testing without one is that a
//! client turns an `eth_call` response into a topology, and that it says so
//! plainly when the endpoint does not answer with one.

use std::net::SocketAddr;

use alloy_primitives::{Address, FixedBytes, U256};
use alloy_sol_types::{sol, SolValue};
use erebus_chain::{ChainError, ChainRegistry};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const CONTRACT: &str = "0x000000000000000000000000000000000000c0de";

sol! {
    struct Node {
        bytes32 key;
        string endpoint;
        uint256 stake;
        address operator;
        uint64 withdrawableAt;
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn snapshot(epoch: u64, seed: [u8; 32], nodes: Vec<(u8, &str, u128)>) -> String {
    let nodes: Vec<Node> = nodes
        .into_iter()
        .map(|(key, endpoint, stake)| Node {
            key: FixedBytes::from([key; 32]),
            endpoint: endpoint.to_string(),
            stake: U256::from(stake),
            operator: Address::ZERO,
            withdrawableAt: 0,
        })
        .collect();
    let output = (U256::from(epoch), FixedBytes::from(seed), nodes).abi_encode_params();
    format!(
        r#"{{"jsonrpc":"2.0","id":1,"result":"0x{}"}}"#,
        hex(&output)
    )
}

/// A one-shot HTTP endpoint that answers whatever it is told to, and hands back
/// the request body it was asked with.
async fn endpoint(response: String) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind::<SocketAddr>("127.0.0.1:0".parse().unwrap())
        .await
        .expect("a port");
    let url = format!("http://{}", listener.local_addr().expect("an address"));

    let served = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("a connection");
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let read = socket.read(&mut buffer).await.expect("a request");
            request.extend_from_slice(&buffer[..read]);
            if read == 0 || request.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                    response.len(),
                    response
                )
                .as_bytes(),
            )
            .await
            .expect("a response");
        socket.flush().await.expect("flushed");
        String::from_utf8_lossy(&request).to_string()
    });

    (url, served)
}

#[tokio::test]
async fn a_snapshot_read_over_json_rpc_becomes_a_usable_topology() {
    let (url, served) = endpoint(snapshot(
        42,
        [0xcd; 32],
        vec![
            (1, "127.0.0.1:9000", 1_000),
            (2, "127.0.0.1:9001", 1_000),
            (3, "127.0.0.1:9002", 1_000),
        ],
    ))
    .await;

    let registry = ChainRegistry::new(url, CONTRACT)
        .expect("a client")
        .fetch()
        .await
        .expect("a registry");

    assert_eq!(registry.nodes.len(), 3);
    assert!(registry.epoch_seed.starts_with("42:"));
    // The whole point: what came off the chain drives layer assignment and path
    // selection with no further translation.
    let layers = registry.layers().expect("layers");
    assert_eq!(layers.len(), 3);
    assert!(layers.iter().all(|layer| layer.len() == 1));

    let request = served.await.expect("the endpoint served");
    assert!(request.contains("eth_call"), "not an eth_call: {request}");
    // `snapshot()` and nothing else, at the address it was given.
    assert!(request.contains("0x9711715a"), "wrong selector: {request}");
    assert!(request.to_lowercase().contains(CONTRACT));
}

#[tokio::test]
async fn an_rpc_error_is_reported_as_one() {
    let (url, _served) = endpoint(
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"execution reverted"}}"#
            .to_string(),
    )
    .await;

    let failure = ChainRegistry::new(url, CONTRACT)
        .expect("a client")
        .fetch()
        .await
        .expect_err("an error");

    assert!(
        matches!(&failure, ChainError::Rpc(message) if message.contains("execution reverted")),
        "unexpected error: {failure}"
    );
}

/// An endpoint that answers with something that is not a snapshot must not
/// leave a client with a half-built topology.
#[tokio::test]
async fn output_that_is_not_a_snapshot_is_refused() {
    let (url, _served) =
        endpoint(r#"{"jsonrpc":"2.0","id":1,"result":"0xdeadbeef"}"#.to_string()).await;

    let failure = ChainRegistry::new(url, CONTRACT)
        .expect("a client")
        .fetch()
        .await
        .expect_err("an error");

    assert!(
        matches!(failure, ChainError::BadReturn(_)),
        "unexpected error: {failure}"
    );
}
