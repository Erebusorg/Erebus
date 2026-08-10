import Link from "next/link";

export function Footer() {
  return (
    <footer className="border-t border-line">
      <div className="mx-auto flex max-w-6xl flex-col gap-6 px-6 py-10 text-[13px] text-muted sm:flex-row sm:items-center sm:justify-between">
        <p>
          Erebus — network-layer privacy for tokenized finance. Alpha, testnet
          only.
        </p>
        <div className="flex gap-6">
          <Link href="/paper" className="hover:text-foreground">
            Paper
          </Link>
          <a
            href="https://docs.robinhood.com/chain/"
            className="hover:text-foreground"
            target="_blank"
            rel="noreferrer"
          >
            Robinhood Chain
          </a>
        </div>
      </div>
    </footer>
  );
}
