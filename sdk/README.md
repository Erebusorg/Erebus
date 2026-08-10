# @erebus/sdk

The Erebus client, compiled to WebAssembly, with an EIP-1193 provider on top.

Path selection, packet construction, reply blocks, and reply decryption happen in
the page. The provider is a transport: it holds no keys and signs nothing, so
sign locally and submit with `eth_sendRawTransaction`. What changes is who learns
the transaction came from you.

```ts
import { ErebusClient, ErebusProvider } from "@erebus/sdk";

const client = await ErebusClient.connect({ gateway: "ws://127.0.0.1:8080" });
const provider = new ErebusProvider(client, { destination: "127.0.0.1:9100" });

await provider.request({ method: "eth_blockNumber" });
```

## Build

Needs the Rust toolchain, the `wasm32-unknown-unknown` target, and
[`wasm-pack`](https://rustwasm.github.io/wasm-pack/).

```bash
npm install
npm run build      # wasm-pack -> pkg/, tsc -> dist/
```

## Test

The tests bring up three real mix nodes, a JSON-RPC exit, a gateway, and a stub
chain node, then drive the SDK the way a page would. Nothing is mocked except
the chain.

```bash
npm test
```

## Example

```bash
cd ../mixnet && UPSTREAM=http://127.0.0.1:8545 ./scripts/local-devnet.sh
```

then, in another shell:

```bash
npm run build
npx http-server . -p 8000
open http://127.0.0.1:8000/examples/browser.html
```

## API

| | |
| --- | --- |
| `ErebusClient.connect(options)` | Opens the gateway socket and builds a client from the registry it serves. |
| `client.request(destination, bytes)` | Sends bytes to a destination service and resolves with the answer. |
| `client.send(destination, bytes)` | Sends bytes nothing can answer. |
| `client.probe()` | Times a packet routed back to this client. |
| `client.startCoverTraffic(destination, meanIntervalMs?)` | Sends packets that carry nothing, at exponentially spaced intervals. |
| `client.close()` | Stops cover traffic, rejects anything in flight, closes the socket. |
| `new ErebusProvider(client, { destination, chainId? })` | An EIP-1193 provider over that client. |

`connect` takes `gateway`, and optionally `meanDelayMs` (default 50), `timeoutMs`
(default 20000), `registry`, `wasm`, and `socket` — the last for runtimes with no
global `WebSocket`, such as Node.

## Caveats

- No public gateway and no public node fleet exist. This runs against a devnet
  you start yourself.
- Fees are paid the ordinary way; the shielded fee pool is not built.
- Unaudited.
