import type { Metadata } from "next";
import Link from "next/link";
import { MarkdownPage } from "@/components/markdown-page";

export const metadata: Metadata = {
  title: "Benchmarks — Erebus",
  description:
    "Measured cost of building and processing Sphinx packets, and why a mix node is bound by bandwidth rather than cryptography.",
};

export default function BenchmarksPage() {
  return (
    <MarkdownPage
      slug="benchmarks"
      eyebrow="Benchmarks"
      title="What the packets cost"
    >
      Measured with the bench example in the repository, on one ordinary
      machine. To run it yourself, start from the{" "}
      <Link href="/docs" className="text-accent">
        docs
      </Link>
      .
    </MarkdownPage>
  );
}
