Erebus is two things: a Rust workspace that implements the mixnet, and this site,
which documents it. Nothing here talks to a live network yet — the code runs a
three-node mixnet on your own machine, and every node, key, and route below is
one you created.

## Run a mixnet locally

```bash
git clone https://github.com/Erebusorg/erebus
cd erebus/mixnet
cargo test
./scripts/local-network.sh "buy 10 AAPL"
```

The script generates three node keys, writes a registry, starts a node per
layer, starts an echo service, sends one request through all three hops, and
runs a loop probe back to itself:

```
echo: buy 10 AAPL
probe returned in 131 ms
```

Every packet on every link is exactly 32 KB, whether it carries that message, a
reply, or cover traffic.

## Do it by hand

Each node needs a key and an address:

```bash
cargo run -p erebus-node -- keygen --out entry.key
cargo run -p erebus-node -- run \
  --key entry.key --listen 127.0.0.1:9001 --registry registry.json
```

The registry is a plain JSON file for now, and every client derives the same
layer assignment from it, so there is no directory server to trust:

```json
{
  "epoch_seed": "erebus-devnet-1",
  "nodes": [
    {
      "id": "<32-byte public key, hex>",
      "address": "127.0.0.1:9001",
      "stake": 0
    }
  ]
}
```

Then send something through it:

```bash
cargo run -p erebus-client -- sink --registry registry.json --listen 127.0.0.1:9100
cargo run -p erebus-client -- send \
  --registry registry.json --to 127.0.0.1:9100 --message "buy 10 AAPL"
cargo run -p erebus-client -- probe --registry registry.json
```

The client picks one node per layer, samples an exponential delay for each hop,
builds the packet, and attaches a reply block whose return path is chosen
independently of the forward path. The service answers into that reply block
without ever learning where you are.

## What each hop learns

| Hop         | Learns                                  | Cannot learn                                            |
| ----------- | --------------------------------------- | ------------------------------------------------------- |
| Entry       | Your address, the relay it forwards to  | The exit, the destination, the payload                  |
| Relay       | The entry and the exit                  | Your address, the destination, the payload              |
| Exit        | The relay, the destination, the payload | Your address, the entry                                 |
| Destination | The request, a reply block              | Your address, the path, whether a reply block is reused |

No field of a packet survives a hop: the header element, the routing block, and
the payload are all re-randomized, so two hops that compare notes see nothing in
common except the packet size, which is the same for every packet.

## Crates

| Crate             | Responsibility                                                |
| ----------------- | ------------------------------------------------------------- |
| `erebus-sphinx`   | Packet format: header, layered payload, reply blocks          |
| `erebus-topology` | Registry, layer assignment, path selection, delay sampling    |
| `erebus-wire`     | Fixed-size framing between nodes                              |
| `erebus-node`     | The mix node: peel, delay, forward, reject replays            |
| `erebus-client`   | Requests, reply blocks, cover traffic, loop probes, demo sink |

## Not built yet

The registry is a file, not a contract; stake is a field nobody enforces; there
is no slashing, no key rotation, and no node-originated cover traffic. The exit
delivers to a plain TCP service rather than to Robinhood Chain JSON-RPC, fees
are not shielded, and there is no browser SDK, so nothing here is usable from a
wallet yet. The [paper](/paper) states which of these are engineering work and
which are open problems.
