import type { Metadata } from "next";
import { MarkdownPage } from "@/components/markdown-page";

export const metadata: Metadata = {
  title: "Erebus — Network-Layer Privacy for Tokenized Finance",
  description:
    "Specification of the Erebus mixnet: Sphinx packets, continuous Poisson mixing, shielded fee payment, on-chain node registry, threat model, and limitations.",
};

export default function PaperPage() {
  return (
    <MarkdownPage
      slug="whitepaper"
      eyebrow="Draft 0.1"
      title="Erebus: Network-Layer Privacy for Tokenized Finance"
    >
      A Sphinx mixnet with shielded fee payment for Robinhood Chain
    </MarkdownPage>
  );
}
