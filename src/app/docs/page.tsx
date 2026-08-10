import type { Metadata } from "next";
import Link from "next/link";
import { Mark } from "@/components/mark";
import { Prose } from "@/components/prose";
import { loadMarkdown } from "@/lib/content";

export const metadata: Metadata = {
  title: "Docs — Erebus",
  description:
    "Run the Erebus mixnet locally: three nodes, a registry, a request through all three hops, and what each hop learns.",
};

export default async function DocsPage() {
  const body = await loadMarkdown("docs");

  return (
    <article className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <header className="border-b border-line pb-10">
        <div className="flex items-center gap-3">
          <Mark size={22} className="text-accent" />
          <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
            Docs · devnet
          </p>
        </div>
        <h1 className="mt-6 text-3xl leading-tight tracking-[-0.02em] sm:text-5xl">
          Run the mixnet
        </h1>
        <p className="mt-5 text-[15px] text-muted">
          Three hops on your own machine, in about a minute. For the design
          behind it, read the{" "}
          <Link href="/paper" className="text-accent">
            paper
          </Link>
          .
        </p>
      </header>

      <div className="mt-14">
        <Prose>{body}</Prose>
      </div>
    </article>
  );
}
