## What it is

`@erebus/sdk` is the client, compiled to WebAssembly. It picks the path, picks
the per-hop delays, builds the Sphinx packet, builds the reply block, and opens
the reply — all of it on your machine, in the page. On top of that sits an
EIP-1193 provider, so anything that already speaks to a wallet can speak through
the mixnet instead.

It is a transport. It holds no keys and signs nothing. Sign locally, submit with
`eth_sendRawTransaction`, and what changes is not who signed the transaction but
who learns it came from you.

## Install

The package is not on npm yet. Build it from the repository:

```bash
git clone https://github.com/Erebusorg/erebus
cd erebus/sdk
npm install
npm run build     # wasm-pack + tsc
```

`npm run build` needs the Rust toolchain, the `wasm32-unknown-unknown` target,
and [`wasm-pack`](https://rustwasm.github.io/wasm-pack/).

## Use it

```ts
import { ErebusClient, ErebusProvider } from "@erebus/sdk";

const client = await ErebusClient.connect({
  gateway: "ws://127.0.0.1:8080",
  meanDelayMs: 50,
});

const provider = new ErebusProvider(client, {
  destination: "127.0.0.1:9100",
});

const blockNumber = await provider.request({ method: "eth_blockNumber" });
```

`provider` satisfies EIP-1193: `request`, `on`, and `removeListener`. Hand it to
viem or ethers as a transport and the library will not notice the difference.

```ts
import { createPublicClient, custom } from "viem";

const chain = createPublicClient({ transport: custom(provider) });
```

### Cover traffic

Traffic that only appears when you have something to say is a signal in itself.
The client can send packets that carry nothing, at exponentially spaced
intervals, so that sending is not the tell:

```ts
client.startCoverTraffic("127.0.0.1:9100", 5_000);
```

Cover packets are the same size as real ones, take the same kind of path, and
are discarded at the exit.

### Loop probes

A probe is a packet addressed back to you. It returns only if every hop on the
path forwarded it, which is what makes silent dropping detectable rather than
free:

```ts
const roundTripMs = await client.probe();
```

## The gateway

A page cannot open a raw socket and cannot be dialled, so it cannot hand a packet
to an entry node and cannot be the address a reply is delivered to. A gateway
does both on its behalf.

The gateway sees that you are speaking to the mixnet and which entry node you
chose. Your own network link sees that anyway. It does not see the rest of the
path, the destination, the request, or the reply: the packet is encrypted to
hops it has no key for, and the reply is sealed under a key only your page holds.
It routes replies by the reply-block id you registered, and drops anything nobody
is waiting for.

## Run the whole thing locally

```bash
cd mixnet
UPSTREAM=http://127.0.0.1:8545 ./scripts/local-devnet.sh
```

That brings up three mix nodes, a JSON-RPC exit pointed at `UPSTREAM`, and a
gateway on `ws://127.0.0.1:8080`. Then serve the example page:

```bash
cd sdk
npm run build
npx http-server . -p 8000     # or any static server
open http://127.0.0.1:8000/examples/browser.html
```

## What the exit will carry

An exit forwards a fixed list of JSON-RPC methods and refuses everything else,
so it cannot be used as an open proxy and its operator is not asked to relay
arbitrary traffic for strangers. Anything outside the list comes back as
`-32601`:

```text
eth_blockNumber   eth_call             eth_chainId
eth_estimateGas   eth_feeHistory       eth_gasPrice
eth_getBalance    eth_getBlockByNumber eth_getCode
eth_getLogs       eth_getTransactionByHash
eth_getTransactionCount                eth_getTransactionReceipt
eth_maxPriorityFeePerGas               eth_sendRawTransaction
net_version
```

Methods that need a key — `eth_sendTransaction`, `personal_sign`,
`wallet_switchEthereumChain`, and the rest — are refused by the provider before
anything is sent, with EIP-1193 code `4200`.

## What this does not do yet

- There is no public gateway and no public node fleet. Everything above runs
  against a devnet you start yourself.
- Fees are paid the ordinary way. The shielded fee pool is not built.
- Nothing here is audited.
