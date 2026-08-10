# Erebus mixnet

The transport layer described in [`content/whitepaper.md`](../content/whitepaper.md),
implemented: Sphinx packets, three mix layers, per-hop exponential delays, reply
blocks, cover traffic, and replay rejection.

Nothing here touches a chain yet. A packet's exit hands the payload to a
destination service over TCP; swapping that service for one that speaks JSON-RPC
to Robinhood Chain is the next phase, not this one.

## Crates

| Crate | What it is |
| --- | --- |
| `erebus-sphinx` | The packet format. Fixed 32 KB, one layer per hop, no field shared between hops. |
| `erebus-topology` | Registry, deterministic layer assignment, path selection, exponential delays. |
| `erebus-wire` | Link framing: bare constant-size frames between mix nodes, length-prefixed delivery out of the network. |
| `erebus-node` | The mix node: peel, hold, forward. |
| `erebus-client` | Path selection, reply blocks, loop probes, cover traffic, and a demo destination service. |

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

A registry is a JSON file. On chain it becomes a contract; the client code does
not change, because layer assignment and path selection already derive from
public data only.

```json
{
  "epoch_seed": "block-hash-of-the-epoch",
  "nodes": [
    { "id": "<hex x25519 public key>", "address": "127.0.0.1:9000", "stake": 1 }
  ]
}
```

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
- **No registry contract.** The node set is a file that every participant is
  trusted to have the same copy of.
- **No stake, no slashing.** `stake` is carried and ignored.
- **Cover traffic is client-side only.** Nodes do not yet originate loops of
  their own, so a network with few clients has little cover.
- **Delays are honoured, not proven.** Loop probes detect a node that drops
  packets. They do not yet detect one that forwards immediately instead of
  waiting.
