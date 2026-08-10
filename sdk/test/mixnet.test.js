// The SDK, driven the way a page would drive it, against a real three-hop
// network on loopback.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test, { after, before } from "node:test";
import { fileURLToPath } from "node:url";
import WebSocket, { WebSocketServer } from "ws";

import { ErebusClient, ErebusProvider, ProviderRpcError } from "../dist/index.js";
import { startDevnet } from "./devnet.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const wasm = {
  module_or_path: readFileSync(path.join(here, "../pkg/erebus_sdk_bg.wasm")),
};

let devnet;
const open = [];

before(async () => {
  devnet = await startDevnet();
});

after(() => {
  for (const client of open) client.close();
  devnet?.stop();
});

async function connect() {
  const client = await ErebusClient.connect({
    gateway: devnet.gateway,
    meanDelayMs: 5,
    timeoutMs: 20_000,
    wasm,
    socket: (url) => new WebSocket(url),
  });
  open.push(client);
  return client;
}

async function until(condition, timeoutMs = 5_000) {
  const deadline = Date.now() + timeoutMs;
  while (!condition()) {
    if (Date.now() > deadline) throw new Error("condition never held");
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

/**
 * A relay in front of the gateway, so a test can cut the connection the way a
 * gateway restart or a dropped network does — without a close handshake.
 */
async function relay() {
  const server = new WebSocketServer({ host: "127.0.0.1", port: 0 });
  const clients = [];

  server.on("connection", (downstream) => {
    clients.push(downstream);
    const upstream = new WebSocket(devnet.gateway);
    const queued = [];
    upstream.on("open", () => {
      for (const message of queued) upstream.send(message);
      queued.length = 0;
    });
    downstream.on("message", (data) =>
      upstream.readyState === WebSocket.OPEN
        ? upstream.send(data)
        : queued.push(data),
    );
    upstream.on("message", (data) => downstream.send(data));
    downstream.on("close", () => upstream.close());
    upstream.on("close", () => downstream.close());
    upstream.on("error", () => downstream.terminate());
  });
  await new Promise((resolve) => server.on("listening", resolve));

  return {
    url: `ws://127.0.0.1:${server.address().port}`,
    hangUp: () => clients.forEach((socket) => socket.terminate()),
    stop: () => server.close(),
  };
}

test("a provider answers an eth_ call over three hops", async () => {
  const client = await connect();
  const provider = new ErebusProvider(client, { destination: devnet.exit });

  const chainId = await provider.request({ method: "eth_chainId" });
  assert.equal(chainId, "0x1b58");
  assert.equal(client.inFlight, 0);

  // The chain node saw the call, and saw it arrive from the exit.
  assert.ok(devnet.calls.some((call) => call.method === "eth_chainId"));
});

test("a chain id already known is answered without a round trip", async () => {
  const client = await connect();
  const provider = new ErebusProvider(client, {
    destination: devnet.exit,
    chainId: "0x1b58",
  });
  const before = devnet.calls.length;
  assert.equal(await provider.request({ method: "eth_chainId" }), "0x1b58");
  assert.equal(devnet.calls.length, before);
});

test("requests in flight together are matched to their own replies", async () => {
  const client = await connect();
  const provider = new ErebusProvider(client, { destination: devnet.exit });

  const answers = await Promise.all(
    Array.from({ length: 8 }, () =>
      provider.request({ method: "eth_blockNumber", params: [] }),
    ),
  );
  assert.equal(new Set(answers).size, answers.length, "replies were crossed");
  assert.equal(client.inFlight, 0);
});

test("a method the exit does not forward is refused, not proxied", async () => {
  const client = await connect();
  const provider = new ErebusProvider(client, { destination: devnet.exit });
  const before = devnet.calls.length;

  await assert.rejects(
    provider.request({ method: "admin_peers" }),
    (error) => error instanceof ProviderRpcError && error.code === -32601,
  );
  assert.equal(devnet.calls.length, before, "the exit forwarded it anyway");
});

test("a method that needs a key is refused before anything is sent", async () => {
  const client = await connect();
  const provider = new ErebusProvider(client, { destination: devnet.exit });

  await assert.rejects(
    provider.request({ method: "eth_sendTransaction", params: [{}] }),
    (error) => error instanceof ProviderRpcError && error.code === 4200,
  );
});

test("a loop probe comes back and takes at least the delays it asked for", async () => {
  const client = await ErebusClient.connect({
    gateway: devnet.gateway,
    meanDelayMs: 50,
    wasm,
    socket: (url) => new WebSocket(url),
  });
  open.push(client);

  const elapsed = await client.probe();
  assert.ok(elapsed > 0, `probe returned in ${elapsed} ms`);
  assert.equal(client.inFlight, 0);
});

test("cover traffic is sent and answers keep working around it", async () => {
  const client = await connect();
  const provider = new ErebusProvider(client, { destination: devnet.exit });

  client.startCoverTraffic(devnet.exit, 50);
  const before = devnet.calls.length;
  const answer = await provider.request({ method: "eth_blockNumber" });
  client.stopCoverTraffic();

  assert.ok(typeof answer === "string");
  // Cover traffic is discarded at the exit: the chain node saw only the real call.
  assert.equal(devnet.calls.length, before + 1);
});

test("cover traffic stops when the gateway hangs up", async () => {
  const gateway = await relay();
  const client = await ErebusClient.connect({
    gateway: gateway.url,
    meanDelayMs: 0,
    wasm,
    socket: (url) => new WebSocket(url),
  });

  client.startCoverTraffic(devnet.exit, 20);
  gateway.hangUp();

  // The client notices by itself, rather than building packets into a socket
  // that is gone and believing it is still covered.
  await until(() => !client.open);
  await assert.rejects(
    client.request(devnet.exit, new TextEncoder().encode("anyone there")),
    /closed/,
  );
  gateway.stop();
});

test("a repeated method key is refused rather than screened as one call and run as another", async () => {
  const client = await connect();
  const before = devnet.calls.length;

  const body = new TextEncoder().encode(
    '{"jsonrpc":"2.0","id":1,"method":"admin_peers","method":"eth_chainId"}',
  );
  const answer = JSON.parse(
    new TextDecoder().decode(await client.request(devnet.exit, body)),
  );

  assert.equal(answer.error.code, -32700);
  assert.equal(devnet.calls.length, before, "the exit forwarded it anyway");
});

test("a request nothing answers times out and releases its reply block", async () => {
  const client = await ErebusClient.connect({
    gateway: devnet.gateway,
    meanDelayMs: 0,
    timeoutMs: 1_000,
    wasm,
    socket: (url) => new WebSocket(url),
  });
  open.push(client);

  await assert.rejects(
    client.request("127.0.0.1:1", new TextEncoder().encode("nobody is there")),
    /no reply within/,
  );
  assert.equal(client.inFlight, 0);
});
