Numbers measured on one machine, on one day, with the code in this repository.
Reproduce them rather than trusting them:

```bash
cd mixnet
cargo run --release -p erebus-sphinx --example bench
```

## Packet operations

Two thousand samples each, after two hundred warm-up iterations, on an Intel
Xeon Platinum 8559C (2 vCPU) running Ubuntu 22.04 and rustc 1.97.1.

| Operation            | Mean   | p50    | p99     | Rate          |
| -------------------- | ------ | ------ | ------- | ------------- |
| Build a 3-hop packet | 241 µs | 215 µs | 1.19 ms | ~4,200/s      |
| Process one hop      | 57 µs  | 53 µs  | 67 µs   | ~17,500/s     |
| Decode from the wire | 0.7 µs | 0.7 µs | 0.7 µs  | ~1,500,000/s  |
| Replay tag           | 0.1 µs | 0.1 µs | 0.1 µs  | ~13,500,000/s |

Building costs about four times a single hop because the client performs the
key agreement for all three hops and pads the routing chain for the full path;
a node only unwraps its own layer. Per-hop cost is what matters for an operator:
one core of this machine will process on the order of seventeen thousand packets
per second, which at 32 KB per packet is far more than the network link under it
can carry. **A mix node is bound by bandwidth, not by cryptography.**

The p99 on the build row is an artefact of a two-vCPU cloud instance sharing a
scheduler, not of the code; the p50 is the honest figure.

## End to end

`mixnet/scripts/local-network.sh` starts three nodes and a destination service
on loopback and sends a real packet through all three hops:

```text
echo: buy 10 AAPL
probe returned in 301 ms
```

That 301 ms is almost entirely the mixing delay we asked for. A loop probe
traverses six hops — three out, three back — at a mean of 50 ms each, so the
expected round trip is 300 ms. Cryptography and loopback transport account for
roughly a millisecond of it.

## What these numbers do not tell you

They are loopback numbers. A real path crosses three machines on the public
internet, so add their round-trip times to the mixing delay, and expect the
1–5 second range the paper quotes rather than 300 ms once delays are tuned for
an anonymity set worth having.

They also say nothing about privacy. Mixing delay is a cost paid deliberately:
a faster mixnet with the same traffic volume is a weaker one. Throughput is an
engineering number, and the security argument lives in the
[paper](/paper) instead.
