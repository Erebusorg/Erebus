import type { Metadata } from "next";
import Link from "next/link";
import { Mark } from "@/components/mark";
import { Prose } from "@/components/prose";
import { loadMarkdown } from "@/lib/content";

export const metadata: Metadata = {
  title: "Benchmarks — Erebus",
  description:
    "Measured cost of building and processing Sphinx packets, and why a mix node is bound by bandwidth rather than cryptography.",
};

export default async function BenchmarksPage() {
  const body = await loadMarkdown("benchmarks");

  return (
    <article className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <header className="border-b border-line pb-10">
        <div className="flex items-center gap-3">
          <Mark size={22} className="text-accent" />
          <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
            Benchmarks
          </p>
        </div>
        <h1 className="mt-6 text-3xl leading-tight tracking-[-0.02em] sm:text-5xl">
          What the packets cost
        </h1>
        <p className="mt-5 text-[15px] text-muted">
          Measured with the bench example in the repository, on one ordinary
          machine. To run it yourself, start from the{" "}
          <Link href="/docs" className="text-accent">
            docs
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
