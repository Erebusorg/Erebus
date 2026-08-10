import type { Metadata } from "next";
import Link from "next/link";
import { Mark } from "@/components/mark";
import { Prose } from "@/components/prose";
import { loadMarkdown } from "@/lib/content";

export const metadata: Metadata = {
  title: "SDK — Erebus",
  description:
    "The Erebus client in WebAssembly, with an EIP-1193 provider: send Ethereum JSON-RPC through three mix hops from a browser.",
};

export default async function SdkPage() {
  const body = await loadMarkdown("sdk");

  return (
    <article className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <header className="border-b border-line pb-10">
        <div className="flex items-center gap-3">
          <Mark size={22} className="text-accent" />
          <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
            SDK · browser
          </p>
        </div>
        <h1 className="mt-6 text-3xl leading-tight tracking-[-0.02em] sm:text-5xl">
          An EIP-1193 provider that routes through the mixnet
        </h1>
        <p className="mt-5 text-[15px] text-muted">
          The same Rust that runs a node builds your packets, in the page. To run
          the network it talks to, see the{" "}
          <Link href="/docs" className="text-accent">
            devnet guide
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
