"use client";

import { useState } from "react";

type UseCase = {
  tab: string;
  eyebrow: string;
  title: string;
  body: string;
  specs: { k: string; v: string }[];
};

const cases: UseCase[] = [
  {
    tab: "Private equity trading",
    eyebrow: "Primary application",
    title: "Trade tokenized stocks without publishing your book",
    body: "A stock token is an ERC-20, so every buy, sell, and holding size sits in public state next to an address that also holds your payroll deposits. Erebus keeps the position in a shielded pool: the client builds a proof locally, the mixnet carries it, and an exit node lands it on Robinhood Chain. The venue settles the trade without learning who took it.",
    specs: [
      { k: "State model", v: "ZK-UTXO commitment tree (depth 32)" },
      { k: "Operations", v: "Deposit, withdraw, transfer, swap" },
      { k: "Fee payment", v: "Shielded; payer identity hidden" },
      { k: "Venue access", v: "Adaptor contracts per DEX/AMM" },
      { k: "Execution", v: "Atomic verify + execute on-chain" },
    ],
  },
  {
    tab: "Private reads",
    eyebrow: "Read path",
    title: "Query the chain without naming what you hold",
    body: "Every RPC call hands a provider your address, your slot, your IP, and your timing — enough to reconstruct a portfolio without a single trade being deanonymized. Erebus routes reads through the mixnet, so the provider answers an exit node instead of you. Session correlation breaks at every request.",
    specs: [
      { k: "Balances", v: "Any address, any stock token" },
      { k: "Contract state", v: "Storage slots and view calls" },
      { k: "Event logs", v: "Historical and streaming" },
      { k: "Cost", v: "Free — reads carry no relay fee" },
    ],
  },
  {
    tab: "Wallet infrastructure",
    eyebrow: "Integration",
    title: "Private by default, one layer below the wallet",
    body: "Erebus sits under the provider interface, not next to it. A wallet swaps its EIP-1193 transport for the Erebus SDK and every call it already makes is mixed before it reaches a node. No new screens, no new approvals, no change in how a user signs.",
    specs: [
      { k: "RPC routing", v: "All calls through the mixnet" },
      { k: "Broadcast", v: "Exit node submits; wallet hidden" },
      { k: "Provider view", v: "Sees an exit node, never the user" },
      { k: "Integration", v: "SDK drop-in, no UX change" },
      { k: "Correlation", v: "Broken per request" },
    ],
  },
];

export function UseCases() {
  const [active, setActive] = useState(0);
  const c = cases[active];

  return (
    <div>
      <div className="flex flex-wrap gap-x-6 gap-y-2 border-b border-line pb-4">
        {cases.map((item, i) => (
          <button
            key={item.tab}
            type="button"
            onClick={() => setActive(i)}
            className={`text-[13px] transition-colors ${
              i === active
                ? "text-foreground"
                : "text-muted hover:text-foreground"
            }`}
          >
            {item.tab}
          </button>
        ))}
      </div>

      <div key={c.tab} className="reveal mt-10 grid gap-12 md:grid-cols-2" data-shown="true">
        <div>
          <p className="font-mono text-[11px] tracking-[0.18em] uppercase text-accent">
            {c.eyebrow}
          </p>
          <h3 className="mt-4 text-2xl leading-tight tracking-tight sm:text-3xl">
            {c.title}
          </h3>
          <p className="mt-5 text-[15px] leading-relaxed text-muted">{c.body}</p>
        </div>

        <dl className="divide-y divide-line border-t border-line">
          {c.specs.map((s) => (
            <div key={s.k} className="flex justify-between gap-6 py-3.5">
              <dt className="text-[13px] text-muted">{s.k}</dt>
              <dd className="text-right font-mono text-[13px]">{s.v}</dd>
            </div>
          ))}
        </dl>
      </div>
    </div>
  );
}
