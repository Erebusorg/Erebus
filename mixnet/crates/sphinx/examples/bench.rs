//! Throughput of the packet operations, measured rather than estimated.
//!
//! ```text
//! cargo run --release -p erebus-sphinx --example bench
//! ```
//!
//! Reports the cost of building a three-hop packet, of one hop processing it,
//! and of the replay tag, so the numbers on the website can be reproduced.

use std::time::{Duration, Instant};

use erebus_sphinx::{Packet, PathHop, PrivateKey, Processed};

const WARMUP: usize = 200;
const SAMPLES: usize = 2_000;
const MESSAGE: &[u8] = b"eth_sendRawTransaction";
const TAG: [u8; 32] = *b"127.0.0.1:9100\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

fn measure(label: &str, samples: usize, mut op: impl FnMut()) {
    for _ in 0..WARMUP {
        op();
    }

    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        op();
        timings.push(start.elapsed());
    }
    timings.sort_unstable();

    let total: Duration = timings.iter().sum();
    let mean = total / samples as u32;
    println!(
        "{label:<28} mean {:>9.1} us   p50 {:>9.1} us   p99 {:>9.1} us   {:>10.0} op/s",
        mean.as_secs_f64() * 1e6,
        timings[samples / 2].as_secs_f64() * 1e6,
        timings[samples * 99 / 100].as_secs_f64() * 1e6,
        1.0 / mean.as_secs_f64(),
    );
}

fn main() {
    let nodes: Vec<PrivateKey> = (0..3).map(|_| PrivateKey::random()).collect();
    let path: Vec<PathHop> = nodes
        .iter()
        .map(|node| PathHop {
            id: node.public().to_bytes(),
            delay_ms: 50,
        })
        .collect();

    let packet = Packet::build(MESSAGE, &path, TAG).expect("build");
    let bytes = packet.to_bytes();

    measure("build (3 hops)", SAMPLES, || {
        Packet::build(MESSAGE, &path, TAG).expect("build");
    });

    measure("process (1 hop)", SAMPLES, || {
        match nodes[0].process(&packet).expect("process") {
            Processed::Forward { .. } => {}
            _ => panic!("expected a forward"),
        }
    });

    measure("decode from wire", SAMPLES, || {
        Packet::from_bytes(&bytes).expect("decode");
    });

    measure("replay tag", SAMPLES * 5, || {
        let _ = PrivateKey::replay_tag(&packet);
    });

    println!("\npacket size: {} bytes", bytes.len());
}
