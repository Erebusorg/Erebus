import Link from "next/link";
import { Mark } from "@/components/mark";
import { MixnetBackdrop } from "@/components/mixnet-backdrop";
import { SectionLabel } from "@/components/page-header";
import { Reveal } from "@/components/reveal";
import { Topology } from "@/components/topology";
import { UseCases } from "@/components/use-cases";

const pillars = [
  {
    n: "01",
    title: "Hides your address",
    lede: "Your real network address never touches the chain.",
    body: "Every packet is onion-encrypted for three independent mix nodes. Each hop peels one layer, holds the packet for a random delay, and forwards something no observer can match to what arrived.",
  },
  {
    n: "02",
    title: "Pays without a trace",
    lede: "Cover the relay fee without revealing who is paying.",
    body: "A zero-knowledge proof shows a valid note is being spent, not whose note it is. Fees settle out of a shielded pool, so nothing on-chain links the payment back to a funding address.",
  },
  {
    n: "03",
    title: "Routes every request",
    lede: "Trades, RPC calls, and balance reads all take the same path.",
    body: "Nothing leaves the client unmixed. Services answer through single-use reply blocks, so a venue or provider can respond to you without ever learning where you are.",
  },
];

export default function Home() {
  return (
    <>
      <section className="relative overflow-hidden border-b border-line">
        <MixnetBackdrop />
        <div className="relative mx-auto max-w-6xl px-6 pt-28 pb-32 sm:pt-40 sm:pb-44">
          <Reveal>
            <div className="flex items-center gap-3">
              <Mark size={30} className="text-accent" />
              <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
                Alpha · Robinhood Chain testnet
              </p>
            </div>
          </Reveal>
          <Reveal delay={80}>
            <h1 className="mt-8 text-6xl leading-[0.95] font-medium tracking-[-0.03em] sm:text-8xl">
              Erebus
            </h1>
          </Reveal>
          <Reveal delay={160}>
            <p className="mt-8 max-w-2xl text-lg leading-relaxed text-muted sm:text-xl">
              Privacy at the network layer for tokenized finance. Erebus hides
              who you are, what you trade, and how you pay, so no observer can
              link any of it back to you.
            </p>
          </Reveal>
          <Reveal delay={240}>
            <div className="mt-12 flex flex-wrap gap-3">
              <Link
                href="/paper"
                className="border border-foreground px-6 py-3 text-[13px] tracking-wide transition-colors hover:bg-foreground hover:text-background"
              >
                Read the paper
              </Link>
              <Link
                href="/docs"
                className="border border-line px-6 py-3 text-[13px] tracking-wide text-muted transition-colors hover:border-muted hover:text-foreground"
              >
                Run it locally
              </Link>
            </div>
          </Reveal>
        </div>
      </section>

      <Section id="what" index="01" label="What Erebus does">
        <div className="grid gap-px bg-line sm:grid-cols-3">
          {pillars.map((p, i) => (
            <Reveal key={p.n} delay={i * 90}>
              <article className="flex h-full flex-col bg-background p-8">
                <span className="font-mono text-[11px] tracking-[0.2em] text-accent">
                  {p.n}
                </span>
                <h3 className="mt-8 text-xl tracking-tight">{p.title}</h3>
                <p className="mt-3 text-[15px] text-foreground/80">{p.lede}</p>
                <p className="mt-5 text-[14px] leading-relaxed text-muted">
                  {p.body}
                </p>
              </article>
            </Reveal>
          ))}
        </div>
      </Section>

      <Section id="mixing" index="02" label="How mixing works">
        <div className="grid gap-12 md:grid-cols-2">
          <Reveal>
            <h2 className="text-3xl leading-tight tracking-tight sm:text-4xl">
              Continuous
              <br />
              <span className="text-muted">Poisson mixing</span>
            </h2>
          </Reveal>
          <Reveal delay={90}>
            <div>
              <p className="text-[15px] leading-relaxed text-muted">
                Each packet enters a delay queue at every hop and leaves after
                an independent exponential wait. Output order carries no
                information about input order, so an adversary watching every
                link still cannot pair what went in with what came out. Real
                traffic, loop probes, and drop cover are indistinguishable on
                the wire.
              </p>
              <ul className="mt-8 flex flex-wrap gap-2">
                {[
                  "3-hop stratified topology",
                  "Fixed 32 KB packets",
                  "Exponential delay per hop",
                  "Loop-probe cover traffic",
                ].map((chip) => (
                  <li
                    key={chip}
                    className="border border-line px-3 py-1.5 font-mono text-[11px] text-muted"
                  >
                    {chip}
                  </li>
                ))}
              </ul>
            </div>
          </Reveal>
        </div>
      </Section>

      <Section id="use-cases" index="03" label="Use cases">
        <UseCases />
      </Section>

      <Section id="topology" index="04" label="Network">
        <Reveal>
          <h2 className="max-w-3xl text-3xl leading-tight tracking-tight sm:text-4xl">
            Three layers of independent mix nodes, one drawn from each per path
          </h2>
        </Reveal>
        <Reveal delay={90}>
          <p className="mt-6 max-w-2xl text-[15px] leading-relaxed text-muted">
            Clients build Sphinx packets locally and route them through the
            entry, relay, and exit layers. Nodes are discovered from an on-chain
            registry where operators stake and publish their keys. Layer
            assignment is deterministic, so every client derives the same
            topology with no coordination and no directory server to trust.
          </p>
        </Reveal>
        <Reveal delay={120}>
          <p className="mt-4 text-[15px]">
            <Link href="/network" className="text-accent">
              Network status
            </Link>
            <span className="text-muted">
              {" "}
              — the registry on Robinhood Chain testnet, read live.
            </span>
          </p>
        </Reveal>
        <Reveal delay={160}>
          <div className="mt-14 border border-line p-6 sm:p-10">
            <Topology />
          </div>
        </Reveal>
        <div className="mt-px grid gap-px bg-line sm:grid-cols-3">
          {[
            { k: "Discovery", v: "On-chain registry" },
            { k: "Sybil resistance", v: "Operator stake + slashing" },
            { k: "Path selection", v: "Client-side, per request" },
          ].map((s) => (
            <div key={s.k} className="bg-background p-6">
              <p className="font-mono text-[11px] tracking-[0.18em] uppercase text-muted">
                {s.k}
              </p>
              <p className="mt-2 text-[15px]">{s.v}</p>
            </div>
          ))}
        </div>
      </Section>

      <section className="border-t border-line">
        <div className="mx-auto max-w-6xl px-6 py-28 sm:py-36">
          <Reveal>
            <h2 className="text-4xl leading-[1.05] tracking-[-0.02em] sm:text-6xl">
              Private.
              <br />
              <span className="text-muted">Unlinkable.</span>
              <br />
              Unstoppable.
            </h2>
          </Reveal>
          <Reveal delay={120}>
            <div className="mt-14 grid gap-px border-t border-line bg-line sm:grid-cols-3">
              {[
                {
                  href: "/explorer",
                  label: "Follow a packet",
                  note: "Three hops, one delay at a time",
                },
                {
                  href: "/docs",
                  label: "Run the mixnet",
                  note: "Three nodes on your own machine",
                },
                {
                  href: "/faq",
                  label: "What it does not hide",
                  note: "The limits, stated plainly",
                },
              ].map((item) => (
                <Link
                  key={item.href}
                  href={item.href}
                  className="group flex flex-col justify-between gap-8 bg-background p-8 transition-colors hover:bg-foreground/[0.03]"
                >
                  <span className="text-[17px]">{item.label}</span>
                  <span className="flex items-baseline justify-between text-[13px] text-muted">
                    {item.note}
                    <span
                      aria-hidden="true"
                      className="text-accent transition-transform group-hover:translate-x-1"
                    >
                      →
                    </span>
                  </span>
                </Link>
              ))}
            </div>
          </Reveal>
        </div>
      </section>
    </>
  );
}

function Section({
  id,
  index,
  label,
  children,
}: {
  id: string;
  index: string;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <section id={id} className="scroll-mt-16 border-b border-line">
      <div className="mx-auto max-w-6xl px-6 py-24 sm:py-32">
        <div className="mb-14 flex items-baseline justify-between border-b border-line pb-4">
          <SectionLabel>{label}</SectionLabel>
          <span className="font-mono text-[11px] text-accent">{index}</span>
        </div>
        {children}
      </div>
    </section>
  );
}
