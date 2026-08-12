import type { Metadata } from "next";
import Link from "next/link";
import { Topology } from "@/components/topology";
import { chain, explorerAddress, formatEth, readNetwork } from "@/lib/chain";

export const metadata: Metadata = {
  title: "Network — Erebus",
  description:
    "Status of the Erebus mixnet: protocol parameters, layer assignment, and what has to exist before a public fleet can be listed here.",
};

const parameters = [
  { k: "Packet size", v: "32768 bytes", note: "Identical on every link" },
  { k: "Hops per path", v: "3", note: "One node drawn per layer" },
  {
    k: "Mean delay per hop",
    v: "50 ms",
    note: "Exponential, chosen by the client",
  },
  {
    k: "Reply blocks",
    v: "Single use",
    note: "Return path independent of the forward path",
  },
  { k: "Replay window", v: "Per node, in memory", note: "Cleared on restart" },
  {
    k: "Layer assignment",
    v: "Hash of epoch seed and node key",
    note: "Every client derives the same one",
  },
];

const layers = [
  {
    name: "Entry",
    role: "Accepts client packets",
    sees: "Your address",
  },
  {
    name: "Relay",
    role: "Breaks the entry–exit link",
    sees: "Neither end of the path",
  },
  {
    name: "Exit",
    role: "Delivers to the destination",
    sees: "The destination and the payload",
  },
];

const contracts = [
  { name: "NodeRegistry", address: chain.registry },
  { name: "FeePool", address: chain.feePool },
  { name: "SpendVerifier", address: chain.verifier },
];

function short(value: string) {
  return `${value.slice(0, 10)}…${value.slice(-6)}`;
}

export default async function NetworkPage() {
  const live = await readNetwork();

  return (
    <div className="mx-auto max-w-5xl px-6 py-20 sm:py-28">
      <header className="border-b border-line pb-10">
        <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
          Network
        </p>
        <h1 className="mt-6 max-w-3xl text-3xl leading-tight tracking-[-0.02em] sm:text-5xl">
          {live && live.nodes.length > 0
            ? `${live.nodes.length} node${live.nodes.length === 1 ? "" : "s"} in the set`
            : "The contracts are live; no node has staked in yet"}
        </h1>
        <p className="mt-5 max-w-2xl text-[15px] leading-relaxed text-muted">
          The registry and the fee pool are deployed and verified on {chain.name}
          , and everything below the fold is one <code>snapshot()</code> call
          against the registry rather than a number we typed in. What is in that
          set today is a demo: three nodes on one machine, staked and paid for
          real, with loopback endpoints nobody else can route to. Read the
          endpoints in the table and you can see that for yourself — which is the
          point of putting the set on chain. A public fleet is a separate thing,
          and it does not exist yet.
        </p>
        <div className="mt-8 grid gap-px bg-line sm:grid-cols-3">
          {contracts.map((c) => (
            <div key={c.name} className="bg-background p-5">
              <p className="font-mono text-[11px] tracking-[0.18em] uppercase text-muted">
                {c.name}
              </p>
              <a
                href={explorerAddress(c.address)}
                className="mt-2 block font-mono text-[13px] text-accent"
              >
                {short(c.address)}
              </a>
            </div>
          ))}
        </div>
        <p className="mt-4 text-[13px] text-muted">
          Chain {chain.id}, test value only: the spend circuit&apos;s trusted
          setup is reproducible, so anyone can forge a proof against this pool.
        </p>
      </header>

      <section className="mt-16">
        <h2 className="font-mono text-[11px] tracking-[0.24em] uppercase text-muted">
          Read off chain
        </h2>
        {live === null ? (
          <p className="mt-6 max-w-2xl text-[15px] leading-relaxed text-muted">
            The RPC did not answer, so there is nothing to show here rather than
            something invented. Ask it yourself:{" "}
            <code className="font-mono text-[13px]">
              cast call {short(chain.registry)} &quot;snapshot()&quot; --rpc-url{" "}
              {chain.rpc}
            </code>
          </p>
        ) : (
          <>
            <div className="mt-6 grid gap-px bg-line sm:grid-cols-2 lg:grid-cols-4">
              {[
                { k: "Active nodes", v: String(live.nodes.length) },
                { k: "Keys ever registered", v: String(live.registered) },
                { k: "Minimum bond", v: formatEth(live.minStake) },
                { k: "Notes in the pool", v: String(live.notes) },
                { k: "Epoch", v: String(live.epoch) },
                { k: "Epoch length", v: `${live.epochLength} s` },
                { k: "Fee denomination", v: formatEth(live.denomination) },
                {
                  k: "Epoch seed",
                  v:
                    BigInt(live.seed) === 0n
                      ? "not recorded yet"
                      : short(live.seed),
                },
              ].map((s) => (
                <div key={s.k} className="bg-background p-6">
                  <p className="font-mono text-[11px] tracking-[0.18em] uppercase text-muted">
                    {s.k}
                  </p>
                  <p className="mt-2 font-mono text-[14px]">{s.v}</p>
                </div>
              ))}
            </div>
            {live.nodes.length > 0 && (
              <div className="mt-8 overflow-x-auto">
                <table className="w-full border-collapse text-left text-[13px]">
                  <thead>
                    <tr className="font-mono text-[11px] tracking-[0.14em] uppercase text-muted">
                      <th className="border-b border-line py-3 pr-6 font-normal">
                        Key
                      </th>
                      <th className="border-b border-line py-3 pr-6 font-normal">
                        Endpoint
                      </th>
                      <th className="border-b border-line py-3 pr-6 font-normal">
                        Bond
                      </th>
                      <th className="border-b border-line py-3 font-normal">
                        Operator
                      </th>
                    </tr>
                  </thead>
                  <tbody className="font-mono">
                    {live.nodes.map((n) => (
                      <tr key={n.key}>
                        <td className="border-b border-line py-4 pr-6">
                          {short(n.key)}
                        </td>
                        <td className="border-b border-line py-4 pr-6 text-muted">
                          {n.endpoint}
                        </td>
                        <td className="border-b border-line py-4 pr-6 text-muted">
                          {formatEth(n.stake)}
                        </td>
                        <td className="border-b border-line py-4">
                          <a
                            href={explorerAddress(n.operator)}
                            className="text-accent"
                          >
                            {short(n.operator)}
                          </a>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            <p className="mt-4 text-[13px] text-muted">
              Which layer a node lands in is not listed, and not the
              registry&apos;s to decide: every client derives it from the epoch
              seed and the node&apos;s key. Each bond was staked by its own
              operator, and each of those operators has been paid out of the fee
              pool by a spend whose payer the chain does not name —{" "}
              <code>mixnet/scripts/testnet-round.sh</code> is the round that did
              it.
            </p>
          </>
        )}
      </section>

      <section className="mt-16">
        <h2 className="font-mono text-[11px] tracking-[0.24em] uppercase text-muted">
          Layers
        </h2>
        <div className="mt-6 overflow-x-auto">
          <table className="w-full border-collapse text-left text-[14px]">
            <thead>
              <tr className="font-mono text-[11px] tracking-[0.14em] uppercase text-muted">
                <th className="border-b border-line py-3 pr-6 font-normal">
                  Layer
                </th>
                <th className="border-b border-line py-3 pr-6 font-normal">
                  Role
                </th>
                <th className="border-b border-line py-3 font-normal">
                  Learns
                </th>
              </tr>
            </thead>
            <tbody>
              {layers.map((l) => (
                <tr key={l.name}>
                  <td className="border-b border-line py-4 pr-6">{l.name}</td>
                  <td className="border-b border-line py-4 pr-6 text-muted">
                    {l.role}
                  </td>
                  <td className="border-b border-line py-4 text-muted">
                    {l.sees}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="mt-16">
        <h2 className="font-mono text-[11px] tracking-[0.24em] uppercase text-muted">
          Protocol parameters
        </h2>
        <div className="mt-6 grid gap-px bg-line sm:grid-cols-2 lg:grid-cols-3">
          {parameters.map((p) => (
            <div key={p.k} className="bg-background p-6">
              <p className="font-mono text-[11px] tracking-[0.18em] uppercase text-muted">
                {p.k}
              </p>
              <p className="mt-2 text-[15px]">{p.v}</p>
              <p className="mt-1 text-[13px] text-muted">{p.note}</p>
            </div>
          ))}
        </div>
      </section>

      <section className="mt-16">
        <h2 className="font-mono text-[11px] tracking-[0.24em] uppercase text-muted">
          Topology
        </h2>
        <div className="mt-6 border border-line p-6 sm:p-10">
          <Topology />
        </div>
      </section>

      <section className="mt-16 border-t border-line pt-10">
        <h2 className="text-xl tracking-tight">
          What has to exist before this set is a network
        </h2>
        <ol className="mt-6 max-w-2xl list-decimal space-y-3 pl-5 text-[15px] leading-relaxed text-muted">
          <li>
            Operators who are not us. The registry holds each one&apos;s key,
            endpoint, and bond, so the list is not ours to edit — but three keys
            we staked from one machine buy no anonymity, whatever the contract
            says.
          </li>
          <li>
            A reason to run a node. The shielded fee pool now pays the operators
            of a route without naming the payer, but on a trusted setup anyone
            can reproduce — auditable, and unsafe for real money until a
            multi-party ceremony replaces it.
          </li>
          <li>
            Slashing driven by evidence rather than by an arbiter address, so a
            node that drops packets or ignores the delay it was handed loses
            money on proof rather than on judgement.
          </li>
          <li>
            Loop probes reported by independent clients, which is the only
            honest way to measure whether a node is mixing rather than
            forwarding straight through.
          </li>
        </ol>
        <p className="mt-8 text-[13px] text-muted">
          Treat the protocol parameters as constants, not measurements — only the
          on-chain figures are read live. The measured ones live in the{" "}
          <Link href="/benchmarks" className="text-accent">
            benchmarks
          </Link>
          .
        </p>
      </section>
    </div>
  );
}
