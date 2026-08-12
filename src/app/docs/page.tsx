import type { Metadata } from "next";
import Link from "next/link";
import { MarkdownPage } from "@/components/markdown-page";

export const metadata: Metadata = {
  title: "Docs — Erebus",
  description:
    "Run the Erebus mixnet locally: three nodes, a registry, a request through all three hops, and what each hop learns.",
};

export default function DocsPage() {
  return (
    <MarkdownPage slug="docs" eyebrow="Docs · devnet" title="Run the mixnet">
      Three hops on your own machine, in about a minute. For the design behind
      it, read the{" "}
      <Link href="/paper" className="text-accent">
        paper
      </Link>
      .
    </MarkdownPage>
  );
}
