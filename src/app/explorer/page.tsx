import type { Metadata } from "next";
import Link from "next/link";
import { PacketExplorer } from "@/components/packet-explorer";
import { PageHeader } from "@/components/page-header";

export const metadata: Metadata = {
  title: "Explorer — Erebus",
  description:
    "Watch a packet cross three mix layers: the delay each hop holds it for, what that hop can see, and why every link carries the same 32 KB.",
};

export default function ExplorerPage() {
  return (
    <div className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <PageHeader eyebrow="Explorer · simulated" title="Follow one packet">
        This runs entirely in your browser against no network at all: the delays
        are drawn from the same exponential distribution the client uses, and
        the digests are random because that is exactly what a hop sees. To route
        a packet for real, run the{" "}
        <Link href="/docs" className="text-accent">
          local mixnet
        </Link>
        .
      </PageHeader>

      <div className="mt-12">
        <PacketExplorer />
      </div>

      <section className="mt-16 border-t border-line pt-10">
        <h2 className="text-xl tracking-tight">Why this is not a live map</h2>
        <p className="mt-5 text-[15px] leading-relaxed text-muted">
          A map of real traffic is a map of when packets moved, and publishing
          that in real time hands an observer the timing data the mixnet exists
          to destroy. When there is a public fleet, this page will show node
          identities and aggregate health on a delay, never live per-packet
          movement.
        </p>
      </section>
    </div>
  );
}
