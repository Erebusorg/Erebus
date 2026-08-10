// Brings up everything the SDK needs to be exercised for real: three mix nodes,
// a JSON-RPC exit, a gateway, and a stub chain node for the exit to forward to.
//
// Nothing here is mocked except the chain itself, so a passing test means the
// browser path works end to end, not that a fake agreed with a fake.

import { spawn, execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const mixnet = path.resolve(here, "../../mixnet");
const bin = (name) => path.join(mixnet, "target/release", name);

function freePort() {
  const server = net.createServer();
  return new Promise((resolve, reject) => {
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

/** A chain node that answers whatever it is asked, so the test can assert on it. */
async function stubChain() {
  const seen = [];
  const server = createServer((request, response) => {
    let body = "";
    request.on("data", (chunk) => (body += chunk));
    request.on("end", () => {
      const call = JSON.parse(body);
      seen.push(call);
      const result =
        call.method === "eth_chainId" ? "0x1b58" : "0x" + seen.length.toString(16);
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ jsonrpc: "2.0", id: call.id, result }));
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return { server, seen, url: `http://127.0.0.1:${server.address().port}` };
}

async function waitForPort(port, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const open = await new Promise((resolve) => {
      const socket = net.connect(port, "127.0.0.1");
      socket.on("connect", () => socket.end(resolve.bind(null, true)));
      socket.on("error", () => resolve(false));
    });
    if (open) return;
    if (Date.now() > deadline) throw new Error(`nothing listening on ${port}`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

export async function startDevnet() {
  execFileSync("cargo", ["build", "--release", "--quiet"], {
    cwd: mixnet,
    stdio: "inherit",
  });

  const work = mkdtempSync(path.join(tmpdir(), "erebus-sdk-"));
  const chain = await stubChain();

  const ports = {
    nodes: [await freePort(), await freePort(), await freePort()],
    exit: await freePort(),
    gateway: await freePort(),
    deliveries: await freePort(),
  };

  const ids = ports.nodes.map((_, i) =>
    execFileSync(bin("erebus-node"), ["keygen", "--out", path.join(work, `node${i}.key`)])
      .toString()
      .trim(),
  );

  const registry = path.join(work, "registry.json");
  writeFileSync(
    registry,
    JSON.stringify({
      epoch_seed: `sdk-test-${Date.now()}`,
      nodes: ids.map((id, i) => ({
        id,
        address: `127.0.0.1:${ports.nodes[i]}`,
        stake: 1,
      })),
    }),
  );

  const children = [];
  const start = (command, args) =>
    children.push(spawn(command, args, { stdio: "ignore" }));

  ports.nodes.forEach((port, i) =>
    start(bin("erebus-node"), [
      "run",
      "--key",
      path.join(work, `node${i}.key`),
      "--listen",
      `127.0.0.1:${port}`,
      "--registry",
      registry,
    ]),
  );

  start(bin("erebus-client"), [
    "rpc",
    "--registry",
    registry,
    "--listen",
    `127.0.0.1:${ports.exit}`,
    "--upstream",
    chain.url,
  ]);

  start(bin("erebus-gateway"), [
    "--registry",
    registry,
    "--listen",
    `127.0.0.1:${ports.gateway}`,
    "--mix-listen",
    `127.0.0.1:${ports.deliveries}`,
  ]);

  await Promise.all(
    [...ports.nodes, ports.exit, ports.gateway, ports.deliveries].map(waitForPort),
  );

  return {
    gateway: `ws://127.0.0.1:${ports.gateway}`,
    exit: `127.0.0.1:${ports.exit}`,
    calls: chain.seen,
    stop() {
      for (const child of children) child.kill("SIGKILL");
      chain.server.close();
      rmSync(work, { recursive: true, force: true });
    },
  };
}
