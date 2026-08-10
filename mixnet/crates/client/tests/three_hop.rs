//! End to end tests over a real three-hop network on loopback: three node
//! processes' worth of logic, a destination service, and a client, all talking
//! over TCP.

use std::sync::Arc;
use std::time::{Duration, Instant};

use erebus_client::sink::Sink;
use erebus_client::{Client, ClientConfig};
use erebus_node::{MixNode, NodeConfig};
use erebus_sphinx::{Packet, PrivateKey};
use erebus_topology::{encode_id, NodeRecord, Registry};
use erebus_wire as wire;

struct Network {
    registry: Registry,
    sink_address: String,
}

/// Reserves a loopback port by binding and releasing it, so a registry can be
/// written before the nodes that it describes are running.
fn reserve_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    listener.local_addr().expect("local addr").port()
}

/// Starts three mix nodes and an echoing destination service.
async fn start_network(nodes: usize) -> Network {
    let keys: Vec<PrivateKey> = (0..nodes).map(|_| PrivateKey::random()).collect();
    let records: Vec<NodeRecord> = keys
        .iter()
        .map(|key| NodeRecord {
            id: encode_id(&key.public().to_bytes()),
            address: format!("127.0.0.1:{}", reserve_port()),
            stake: 1,
        })
        .collect();

    let registry = Registry {
        epoch_seed: "test-epoch".to_string(),
        nodes: records.clone(),
    };

    for (key, record) in keys.into_iter().zip(records.iter()) {
        let (_, serve) = MixNode::bind(NodeConfig {
            key,
            listen: record.address.clone(),
            registry: registry.clone(),
        })
        .await
        .expect("node binds");
        tokio::spawn(serve);
    }

    let sink = Sink::new(
        registry.clone(),
        Arc::new(|body: &[u8]| {
            Some(format!("filled: {}", String::from_utf8_lossy(body)).into_bytes())
        }),
    );
    let (sink_address, serve) = sink.bind("127.0.0.1:0").await.expect("sink binds");
    tokio::spawn(serve);

    Network {
        registry,
        sink_address,
    }
}

async fn client(network: &Network, mean_delay_ms: f64) -> Arc<Client> {
    let (client, serve) = Client::bind(ClientConfig {
        registry: network.registry.clone(),
        listen: "127.0.0.1:0".to_string(),
        mean_delay_ms,
    })
    .await
    .expect("client binds");
    tokio::spawn(serve);
    client
}

#[tokio::test]
async fn a_request_reaches_the_service_and_the_reply_comes_back() {
    let network = start_network(3).await;
    let client = client(&network, 0.0).await;

    let reply = client
        .request(
            &network.sink_address,
            b"buy 10 AAPL",
            Duration::from_secs(10),
        )
        .await
        .expect("reply");

    assert_eq!(reply, b"filled: buy 10 AAPL");
}

#[tokio::test]
async fn many_requests_in_flight_are_matched_to_their_own_replies() {
    let network = start_network(6).await;
    let client = client(&network, 5.0).await;

    let mut handles = Vec::new();
    for i in 0..12 {
        let client = Arc::clone(&client);
        let destination = network.sink_address.clone();
        handles.push(tokio::spawn(async move {
            let body = format!("order-{i}");
            let reply = client
                .request(&destination, body.as_bytes(), Duration::from_secs(20))
                .await
                .expect("reply");
            assert_eq!(reply, format!("filled: {body}").into_bytes());
        }));
    }

    for handle in handles {
        handle.await.expect("request task");
    }
}

#[tokio::test]
async fn per_hop_delays_are_actually_applied() {
    let network = start_network(3).await;
    let client = client(&network, 200.0).await;

    let started = Instant::now();
    client
        .request(&network.sink_address, b"slow", Duration::from_secs(30))
        .await
        .expect("reply");
    let elapsed = started.elapsed();

    // Six hops of mean 200 ms: the sum is Gamma(6, 200 ms), so a round trip
    // under 200 ms would mean the delays were not being honoured.
    assert!(
        elapsed > Duration::from_millis(200),
        "round trip took only {elapsed:?}"
    );
}

#[tokio::test]
async fn a_loop_probe_returns_to_the_client_that_sent_it() {
    let network = start_network(3).await;
    let client = client(&network, 0.0).await;

    let elapsed = client
        .loop_probe(Duration::from_secs(10))
        .await
        .expect("probe returns");
    assert!(elapsed < Duration::from_secs(10));
}

#[tokio::test]
async fn cover_traffic_is_accepted_by_the_network_and_dropped_by_the_service() {
    let network = start_network(3).await;
    let client = client(&network, 0.0).await;

    for _ in 0..5 {
        client
            .send_cover(&network.sink_address)
            .await
            .expect("cover sent");
    }

    // Cover traffic must not disturb real traffic that follows it.
    let reply = client
        .request(&network.sink_address, b"real", Duration::from_secs(10))
        .await
        .expect("reply");
    assert_eq!(reply, b"filled: real");
}

#[tokio::test]
async fn a_replayed_packet_is_dropped_by_the_entry_node() {
    let network = start_network(3).await;

    // A packet the client never sent, replayed twice into the entry node: the
    // first copy is delivered, the second must be dropped rather than
    // re-emitted, which is what stops a node being used as an oracle.
    let path = network
        .registry
        .select_path(&mut rand::rngs::OsRng, 0.0)
        .expect("path");
    let entry = network
        .registry
        .address_of(&path[0].id)
        .expect("entry address");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind observer");
    let observed = listener.local_addr().expect("observer addr").to_string();

    let packet = Packet::build(
        b"replay me",
        &path,
        wire::tag_from_address(&observed).expect("tag"),
    )
    .expect("packet");

    wire::send_packet(&entry, &packet)
        .await
        .expect("first copy");
    wire::send_packet(&entry, &packet)
        .await
        .expect("second copy");

    let (mut stream, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
        .await
        .expect("a delivery arrives")
        .expect("accept");
    assert!(wire::read_message(&mut stream)
        .await
        .expect("read")
        .is_some());

    let second = tokio::time::timeout(Duration::from_secs(2), listener.accept()).await;
    assert!(second.is_err(), "the replayed packet was forwarded");
}
