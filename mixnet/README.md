# Erebus mixnet

The transport layer described in [`content/whitepaper.md`](../content/whitepaper.md),
implemented: Sphinx packets, three mix layers, per-hop exponential delays, reply
blocks, cover traffic, and replay rejection.

A packet's exit hands the payload to a destination service over TCP. One such
service speaks Ethereum JSON-RPC: it forwards a fixed list of methods to an
upstream endpoint and refuses everything else, so a page can read the chain and
submit a signed transaction without the endpoint learning who asked.

## Crates

| Crate | What it is |
| --- | --- |
| `erebus-sphinx` | The packet format. Fixed 32 KB, one layer per hop, no field shared between hops. |
| `erebus-topology` | Registry, deterministic layer assignment, path selection, exponential delays. |
| `erebus-wire` | Link framing: bare constant-size frames between mix nodes, length-prefixed delivery out of the network. |
| `erebus-node` | The mix node: peel, hold, forward. |
| `erebus-client` | Path selection, reply blocks, loop probes, cover traffic, an echoing destination service, and the JSON-RPC exit. |
| `erebus-envelope` | What a client puts inside a packet: requests, replies, probes. |
| `erebus-sdk` | The client core with no sockets and no filesystem, so it compiles to WebAssembly. |
| `erebus-gateway` | Carries packets between a browser and the mixnet, without being able to read them. |
| `erebus-chain` | Reads the node set and epoch seed from the registry contract, over one `eth_call`. |

## Run a local network

```bash
cd mixnet
./scripts/local-network.sh "buy 10 AAPL"
```

That generates three node keys, writes a registry, starts three nodes and an
echoing destination service on loopback, sends one message through all three
hops, and then times a packet routed from the client back to itself.

Run the pieces by hand instead:

```bash
cargo run --bin erebus-node -- keygen --out node0.key
cargo run --bin erebus-node -- run --key node0.key --listen 127.0.0.1:9000 --registry registry.json
cargo run --bin erebus-client -- sink --registry registry.json --listen 127.0.0.1:9100
cargo run --bin erebus-client -- send --registry registry.json --to 127.0.0.1:9100 --message "hello"
```

## A devnet a browser can talk to

```bash
UPSTREAM=http://127.0.0.1:8545 ./scripts/local-devnet.sh
```

Three nodes, a JSON-RPC exit pointed at `UPSTREAM`, and a gateway on
`ws://127.0.0.1:8080`, left running. The browser side is in [`../sdk`](../sdk).

The gateway exists because a page cannot open a raw socket and cannot be dialled.
It learns that a client is speaking to the mixnet and which entry node the client
chose — what the client's own network link sees anyway — and nothing more: it
routes replies by the reply-block id a client registers, and drops frames nobody
is waiting for.

## Where the node set comes from

A registry is a JSON file:

```json
{
  "epoch_seed": "block-hash-of-the-epoch",
  "nodes": [
    { "id": "<hex x25519 public key>", "address": "127.0.0.1:9000", "stake": 1 }
  ]
}
```

…or the registry contract in [`../contracts`](../contracts). Every binary that
needs the node set takes either, and nothing downstream of the read changes:

```bash
cargo run --bin erebus-node -- run --key node0.key --listen 127.0.0.1:9000 \
  --chain-rpc https://rpc.testnet.robinhood.com --contract 0xREGISTRY
cargo run --bin erebus-registry -- fetch --rpc $RPC --contract 0xREGISTRY
```

The whole thing against a chain, end to end — a local chain, a deployed
registry, three nodes that stake and register themselves, and a client that
reads the set back off it (needs [foundry](https://getfoundry.sh)):

```bash
./scripts/chain-devnet.sh
```

Layer assignment and path selection derive from public data only, so a client
reading the contract needs no account, signs nothing, and cannot be handed a
different node set than anyone else — which is the point: a directory that can
tailor the set per client has partitioned the anonymity set without being caught.

## What a hop learns

| | Entry | Relay | Exit |
| --- | --- | --- | --- |
| Client IP | yes | no | no |
| Destination | no | no | yes |
| Payload | no | no | yes, unless the request carries a reply block whose reply is sealed end to end |
| That two packets belong to the same client | no | no | no |

## Checks

```bash
cargo test          # unit tests, plus end-to-end tests over loopback
cargo clippy --all-targets
cargo fmt --all --check
```

## Known gaps

- **Payload malleability.** Intermediate hops authenticate the header, not the
  body, so a flipped bit is caught by the exit's AEAD rather than by the hop that
  flipped it. Sphinx has the same property; closing it needs a wide-block cipher.
- **Replay window.** Tags are held in memory and dropped in bulk when the window
  fills. Correct only because node keys are meant to rotate per epoch, which the
  node does not yet do.
- **Stake is recorded, not yet earned back.** The registry bonds a node and can
  slash it, but there are no fees or rewards, so honest operation costs money.
- **Slashing is a judgement, not a proof.** The contract records a decision the
  arbiter made off chain. What a mixnet can actually measure — probes that never
  return — is statistical, and the registry does not pretend otherwise.
- **The epoch seed is not unpredictable to whoever orders blocks.** It is a past
  block hash, which stops an operator picking its own layer, not a sequencer.
- **The node set is read, not watched.** A binary reads the registry at startup;
  it does not yet follow the contract's events and re-derive layers mid-run.
- **Cover traffic is client-side only.** Nodes do not yet originate loops of
  their own, so a network with few clients has little cover.
- **The gateway is a chokepoint.** Every browser using one gateway is visible to
  that gateway as a set of connections, so a gateway with few users is a weak
  anonymity set even though it cannot read a packet. Run your own.
- **Delays are honoured, not proven.** Loop probes detect a node that drops
  packets. They do not yet detect one that forwards immediately instead of
  waiting.
