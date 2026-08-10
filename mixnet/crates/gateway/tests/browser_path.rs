//! The path a browser actually takes: a WebSocket to a gateway, a gateway to
//! three mix nodes, and a destination service that answers into a reply block.
//!
//! The client here is the same [`MixClient`] that is compiled to WebAssembly for
//! the browser, driven over a real socket, so what these tests prove is what a
//! page in a browser gets.

use std::time::Duration;

use erebus_client::sink::{immediate, Sink};
use erebus_envelope::{Frame, Reply};
use erebus_gateway::{Gateway, GatewayConfig};
use erebus_node::{MixNode, NodeConfig};
use erebus_sdk::gateway::{decode_deliver, decode_hello, encode_expect, encode_send};
use erebus_sdk::MixClient;
use erebus_sphinx::PrivateKey;
use erebus_topology::{encode_id, NodeRecord, Registry};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Network {
    registry: Registry,
    sink_address: String,
    websocket: String,
    /// Where the gateway receives mixnet deliveries, as an exit node sees it.
    deliveries: String,
}

fn reserve_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    listener.local_addr().expect("local addr").port()
}

async fn start_network() -> Network {
    let keys: Vec<PrivateKey> = (0..3).map(|_| PrivateKey::random()).collect();
    let records: Vec<NodeRecord> = keys
        .iter()
        .map(|key| NodeRecord {
            id: encode_id(&key.public().to_bytes()),
            address: format!("127.0.0.1:{}", reserve_port()),
            stake: 1,
        })
        .collect();
    let registry = Registry {
        epoch_seed: "browser-epoch".to_string(),
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
        immediate(|body: &[u8]| {
            Some(format!("filled: {}", String::from_utf8_lossy(body)).into_bytes())
        }),
    );
    let (sink_address, serve) = sink.bind("127.0.0.1:0").await.expect("sink binds");
    tokio::spawn(serve);

    let (websocket, deliveries, serve) = Gateway::bind(GatewayConfig {
        listen: "127.0.0.1:0".to_string(),
        mix_listen: "127.0.0.1:0".to_string(),
        advertise: None,
        registry: registry.clone(),
    })
    .await
    .expect("gateway binds");
    tokio::spawn(serve);

    Network {
        registry,
        sink_address,
        websocket,
        deliveries,
    }
}

/// Connects the way a page would, and builds a client from what the gateway
/// said about itself.
async fn connect(network: &Network) -> (Socket, MixClient) {
    let (mut socket, _) = connect_async(format!("ws://{}", network.websocket))
        .await
        .expect("websocket connects");

    let greeting = next_binary(&mut socket).await;
    let hello = decode_hello(&greeting)
        .expect("a greeting")
        .expect("the first message is a greeting");
    let registry: Registry = serde_json::from_str(&hello.registry()).expect("registry json");
    assert_eq!(registry.nodes.len(), network.registry.nodes.len());

    let client = MixClient::new(&hello.registry(), 0.0, &hello.tag()).expect("client");
    (socket, client)
}

async fn next_binary(socket: &mut Socket) -> Vec<u8> {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(10), socket.next())
            .await
            .expect("a message arrives")
            .expect("the socket is open")
            .expect("a readable message");
        if let Message::Binary(bytes) = message {
            return bytes;
        }
    }
}

async fn send(socket: &mut Socket, outgoing: &erebus_sdk::Outgoing) {
    if let Some(id) = outgoing.id() {
        socket
            .send(Message::Binary(encode_expect(&id).expect("expectation")))
            .await
            .expect("registered");
    }
    socket
        .send(Message::Binary(
            encode_send(&outgoing.first_hop(), &outgoing.packet()).expect("send"),
        ))
        .await
        .expect("sent");
}

#[tokio::test]
async fn a_browser_client_gets_its_reply_back_through_the_gateway() {
    let network = start_network().await;
    let (mut socket, mut client) = connect(&network).await;

    let outgoing = client
        .request(&network.sink_address, b"buy 10 AAPL")
        .expect("request");
    send(&mut socket, &outgoing).await;

    let delivered = next_binary(&mut socket).await;
    let frame = decode_deliver(&delivered).expect("a delivery");
    let answer = client.accept(&frame).expect("the reply opens");

    assert_eq!(answer.body().expect("a body"), b"filled: buy 10 AAPL");
    assert_eq!(client.in_flight(), 0);
}

#[tokio::test]
async fn a_loop_probe_comes_back_to_the_browser_that_sent_it() {
    let network = start_network().await;
    let (mut socket, mut client) = connect(&network).await;

    let outgoing = client.probe().expect("probe");
    send(&mut socket, &outgoing).await;

    let delivered = next_binary(&mut socket).await;
    let returned = client
        .accept(&decode_deliver(&delivered).expect("a delivery"))
        .expect("a returning probe");
    assert!(returned.is_probe());
}

#[tokio::test]
async fn two_browsers_do_not_receive_each_others_replies() {
    let network = start_network().await;
    let (mut first, mut first_client) = connect(&network).await;
    let (mut second, second_client) = connect(&network).await;

    let outgoing = first_client
        .request(&network.sink_address, b"first")
        .expect("request");
    send(&mut first, &outgoing).await;

    let delivered = next_binary(&mut first).await;
    let answer = first_client
        .accept(&decode_deliver(&delivered).expect("a delivery"))
        .expect("the reply opens");
    assert_eq!(answer.body().expect("a body"), b"filled: first");

    // The other socket was never told about that reply block, so nothing was
    // written to it.
    let stray = tokio::time::timeout(Duration::from_secs(1), second.next()).await;
    assert!(stray.is_err(), "a reply went to the wrong client");
    assert_eq!(second_client.in_flight(), 0);
}

#[tokio::test]
async fn a_reply_nobody_registered_is_dropped_rather_than_broadcast() {
    let network = start_network().await;
    let (mut socket, client) = connect(&network).await;

    // Delivered into the gateway's mixnet side the way an exit node delivers,
    // under a reply block id no client ever registered.
    let frame = Frame::Reply(Reply {
        surb_id: [3u8; 32],
        sealed: vec![0; 32],
    });
    erebus_wire::send_message(&network.deliveries, &frame.to_bytes())
        .await
        .expect("the gateway accepts the connection");

    let stray = tokio::time::timeout(Duration::from_secs(1), socket.next()).await;
    assert!(stray.is_err(), "an unclaimed reply reached a client");
    assert_eq!(client.in_flight(), 0);
}

#[tokio::test]
async fn a_packet_that_is_not_a_packet_does_not_take_the_gateway_down() {
    let network = start_network().await;
    let (mut socket, mut client) = connect(&network).await;

    socket
        .send(Message::Binary(vec![0x01; 64]))
        .await
        .expect("garbage sent");

    // The socket still works afterwards.
    let outgoing = client
        .request(&network.sink_address, b"still here")
        .expect("request");
    send(&mut socket, &outgoing).await;
    let delivered = next_binary(&mut socket).await;
    let answer = client
        .accept(&decode_deliver(&delivered).expect("a delivery"))
        .expect("the reply opens");
    assert_eq!(answer.body().expect("a body"), b"filled: still here");
}
