import type { Metadata } from "next";
import Link from "next/link";
import { Topology } from "@/components/topology";

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
    operators: "—",
  },
  {
    name: "Relay",
    role: "Breaks the entry–exit link",
    sees: "Neither end of the path",
    operators: "—",
  },
  {
    name: "Exit",
    role: "Delivers to the destination",
    sees: "The destination and the payload",
    operators: "—",
  },
];

export default function NetworkPage() {
  return (
    <div className="mx-auto max-w-5xl px-6 py-20 sm:py-28">
      <header className="border-b border-line pb-10">
        <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
          Network
        </p>
        <h1 className="mt-6 max-w-3xl text-3xl leading-tight tracking-[-0.02em] sm:text-5xl">
          No public network is running yet
        </h1>
        <p className="mt-5 max-w-2xl text-[15px] leading-relaxed text-muted">
          The mixnet runs today only where you start it: three nodes on one
          machine, from the{" "}
          <Link href="/docs" className="text-accent">
            docs
          </Link>
          . The counts below stay empty until nodes are staked in a registry
          contract, because a node list nobody can verify is worth nothing.
        </p>
      </header>

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
                <th className="border-b border-line py-3 pr-6 font-normal">
                  Learns
                </th>
                <th className="border-b border-line py-3 font-normal">
                  Operators
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
                  <td className="border-b border-line py-4 pr-6 text-muted">
                    {l.sees}
                  </td>
                  <td className="border-b border-line py-4 font-mono text-muted">
                    {l.operators}
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
          What has to exist before this page shows live nodes
        </h2>
        <ol className="mt-6 max-w-2xl list-decimal space-y-3 pl-5 text-[15px] leading-relaxed text-muted">
          <li>
            A registry contract on Robinhood Chain holding each operator&apos;s
            key, endpoint, and stake, so the node list is not ours to edit.
          </li>
          <li>
            Slashing, so a node that drops packets or ignores the delay it was
            handed loses money rather than reputation.
          </li>
          <li>
            Loop probes reported by independent clients, which is the only
            honest way to measure whether a node is mixing rather than
            forwarding straight through.
          </li>
        </ol>
        <p className="mt-8 text-[13px] text-muted">
          Until then, treat every number on this page as a protocol constant,
          not a measurement. The measured ones live in the{" "}
          <Link href="/benchmarks" className="text-accent">
            benchmarks
          </Link>
          .
        </p>
      </section>
    </div>
  );
}
