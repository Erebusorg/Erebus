import type { Metadata } from "next";
import Link from "next/link";
import { PageHeader } from "@/components/page-header";

export const metadata: Metadata = {
  title: "FAQ — Erebus",
  description:
    "What Erebus hides, what it does not hide, how much latency mixing costs, and what is still unbuilt.",
};

const faqs = [
  {
    q: "What does Erebus actually hide?",
    a: "Your network address and the relationship between your requests. Nobody on the path sees both who you are and what you asked for: the entry node knows your address but not your request, the exit node knows your request but not your address, and the relay in between knows neither.",
  },
  {
    q: "What does it not hide?",
    a: "Anything already public on-chain. A transaction that settles on Robinhood Chain is as visible as any other; Erebus hides who submitted it and from where, not that it happened. Shielding the positions themselves needs the shielded pool, which is not built yet.",
  },
  {
    q: "Is this a VPN or Tor?",
    a: "No. A VPN moves the trust to one provider that sees everything. Tor is low-latency but does not resist an observer who can watch both ends and correlate timing. Erebus delays every packet independently at every hop and pads everything to one size, which costs latency and buys resistance to exactly that correlation.",
  },
  {
    q: "How slow is it?",
    a: "Expect roughly one to five seconds for a round trip, dominated by the delays you asked for rather than by bandwidth. That is fine for submitting an order or reading a balance and wrong for a live orderbook feed, which is why the design keeps market data out of the mixnet.",
  },
  {
    q: "Can the exit node read my transaction?",
    a: "Yes, and it must, because someone has to hand the transaction to the chain. What it cannot learn is who you are. This is why exit selection is per request and why exit operators need stake at risk.",
  },
  {
    q: "If few people use it, does it still work?",
    a: "Less well, and this is the honest limit of any mixnet: you hide in a crowd, and a small crowd hides you less. Cover traffic raises the floor but does not replace real users. Early adopters get weaker anonymity than the design allows for.",
  },
  {
    q: "Do I have to trust the node operators?",
    a: "Not individually. A path is compromised only if all three hops collude, and layers are assigned from a hash of the epoch seed and the node key, so nobody chooses their own position. What you do have to trust is that the three you drew are not all the same operator wearing different keys, which is what staking is meant to make expensive.",
  },
  {
    q: "Is there a token?",
    a: "No, and there is no sale, no allocation, and no airdrop. Fees are paid in ETH, which is what Robinhood Chain uses for gas.",
  },
  {
    q: "Can I use it today?",
    a: "You can run the mixnet locally and send traffic through three hops. You cannot use it from a wallet: there is no browser SDK and no chain connection yet.",
  },
  {
    q: "Has it been audited?",
    a: "No. Nothing here has been reviewed by anyone outside the project, and the cryptography includes deliberate simplifications documented in the paper. Do not put anything you care about through it.",
  },
];

export default function FaqPage() {
  return (
    <div className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <PageHeader eyebrow="FAQ" title="Questions, answered plainly">
        Including the ones with unflattering answers. The{" "}
        <Link href="/paper" className="text-accent">
          paper
        </Link>{" "}
        has the long form.
      </PageHeader>

      <dl className="mt-6">
        {faqs.map((f) => (
          <details key={f.q} className="group border-b border-line">
            <summary className="flex cursor-pointer items-baseline justify-between gap-6 py-6 text-[16px] transition-colors hover:text-accent">
              <dt>{f.q}</dt>
              <span
                aria-hidden="true"
                className="mt-1 font-mono text-[13px] text-muted transition-transform group-open:rotate-45"
              >
                +
              </span>
            </summary>
            <dd className="pb-7 text-[15px] leading-[1.75] text-muted">
              {f.a}
            </dd>
          </details>
        ))}
      </dl>

      <p className="mt-12 text-[13px] text-muted">
        Something missing?{" "}
        <a
          href="https://github.com/Erebusorg/erebus/issues"
          target="_blank"
          rel="noreferrer"
          className="text-accent underline decoration-accent/40 underline-offset-2"
        >
          Open an issue
        </a>
        .
      </p>
    </div>
  );
}
