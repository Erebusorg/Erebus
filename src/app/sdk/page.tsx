import type { Metadata } from "next";
import Link from "next/link";
import { MarkdownPage } from "@/components/markdown-page";

export const metadata: Metadata = {
  title: "SDK — Erebus",
  description:
    "The Erebus client in WebAssembly, with an EIP-1193 provider: send Ethereum JSON-RPC through three mix hops from a browser.",
};

export default function SdkPage() {
  return (
    <MarkdownPage
      slug="sdk"
      eyebrow="SDK · browser"
      title="An EIP-1193 provider that routes through the mixnet"
    >
      The same Rust that runs a node builds your packets, in the page. To run
      the network it talks to, see the{" "}
      <Link href="/docs" className="text-accent">
        devnet guide
      </Link>
      .
    </MarkdownPage>
  );
}
