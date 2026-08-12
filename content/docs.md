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

## Reading the node set off a chain

A file works on one machine, but a node set that participants have to trust each
other about is exactly the coordination point Erebus is trying not to have. The
same binaries take the registry contract instead:

```bash
cargo run -p erebus-node -- run --key entry.key --listen 127.0.0.1:9001 \
  --chain-rpc https://rpc.testnet.robinhood.com --contract 0xREGISTRY
cargo run -p erebus-registry -- fetch --rpc $RPC --contract 0xREGISTRY
```

The contract is in `contracts/`: a node registers a public key, an endpoint, and
a bond; announcing an exit stops it being selected immediately but keeps the bond
slashable for the unbonding period; and one `snapshot()` call returns the epoch,
its seed, and every node clients should be routing through. Nothing after that
read differs — layer assignment and path selection already derive from public data
alone, so reading the set needs no account and signs nothing.

The whole thing end to end on a local chain, with three nodes that stake and
register themselves ([foundry](https://getfoundry.sh) required):

```bash
cd contracts && forge test
cd ../mixnet && ./scripts/chain-devnet.sh
```

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

## Measure it

```bash
cargo run --release -p erebus-sphinx --example bench
```

Prints the cost of building a packet, of one hop processing it, and of the
replay tag. The numbers from this machine are on the
[benchmarks](/benchmarks) page.

## Not built yet

The registry contract is written and tested but deployed nowhere, so stake is
real only on a chain you start yourself. Nobody is paid for running a node, the
decision to slash is still a human one made off chain, and there is no key
rotation and no node-originated cover traffic. Fees are not shielded, and no
public network is running. A browser can already use all of
this — see the [SDK](/sdk) — but only against a devnet you start yourself. The
[paper](/paper) states which of these are engineering work and which are open
problems.
