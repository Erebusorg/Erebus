import Link from "next/link";
import { Mark } from "@/components/mark";

const columns = [
  {
    heading: "Protocol",
    links: [
      { href: "/paper", label: "Paper" },
      { href: "/network", label: "Network" },
      { href: "/benchmarks", label: "Benchmarks" },
      { href: "/explorer", label: "Explorer" },
      { href: "/faq", label: "FAQ" },
    ],
  },
  {
    heading: "Developers",
    links: [
      { href: "/docs", label: "Docs" },
      { href: "/sdk", label: "SDK" },
      { href: "https://github.com/Erebusorg/erebus", label: "Source code" },
      {
        href: "https://github.com/Erebusorg/erebus/tree/main/mixnet",
        label: "Run a node",
      },
      { href: "/brand", label: "Brand kit" },
    ],
  },
  {
    heading: "Elsewhere",
    links: [
      { href: "https://x.com/Erebusorg", label: "X" },
      { href: "https://docs.robinhood.com/chain/", label: "Robinhood Chain" },
      { href: "/privacy", label: "Privacy" },
      { href: "/terms", label: "Terms" },
      { href: "/content-policy", label: "Content policy" },
    ],
  },
];

const socials = [
  {
    label: "X",
    href: "https://x.com/Erebusorg",
    path: "M13.7 10.6 21.4 2h-1.8l-6.7 7.5L7.6 2H1.4l8 11.4L1.4 22h1.8l7-7.9 5.6 7.9h6.2l-8.3-11.4Zm-2.5 2.8-.8-1.2L3.9 3.3h2.8l5.2 7.4.8 1.2 6.8 9.6h-2.8l-5.5-7.8Z",
  },
  {
    label: "GitHub",
    href: "https://github.com/Erebusorg",
    path: "M12 2a10 10 0 0 0-3.16 19.49c.5.09.68-.22.68-.48l-.01-1.7c-2.78.6-3.37-1.34-3.37-1.34-.45-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.61.07-.61 1 .07 1.53 1.03 1.53 1.03.89 1.53 2.34 1.09 2.91.83.09-.65.35-1.09.63-1.34-2.22-.25-4.56-1.11-4.56-4.94 0-1.09.39-1.99 1.03-2.69-.1-.25-.45-1.27.1-2.65 0 0 .84-.27 2.75 1.03a9.5 9.5 0 0 1 5 0c1.91-1.3 2.75-1.03 2.75-1.03.55 1.38.2 2.4.1 2.65.64.7 1.03 1.6 1.03 2.69 0 3.84-2.34 4.69-4.57 4.94.36.31.68.92.68 1.85l-.01 2.75c0 .26.18.58.69.48A10 10 0 0 0 12 2Z",
  },
];

export function Footer() {
  return (
    <footer className="border-t border-line">
      <div className="mx-auto flex max-w-6xl flex-col gap-12 px-6 py-14 sm:flex-row sm:justify-between">
        <div className="max-w-xs">
          <div className="flex items-center gap-2.5">
            <Mark size={22} className="text-accent" />
            <span className="text-sm font-medium tracking-[0.2em] uppercase">
              Erebus
            </span>
          </div>
          <p className="mt-5 text-[13px] leading-relaxed text-muted">
            Privacy at the network layer for tokenized finance. Alpha: testnet
            only, unaudited, no token.
          </p>
          <ul className="mt-6 flex gap-4">
            {socials.map((social) => (
              <li key={social.href}>
                <a
                  href={social.href}
                  target="_blank"
                  rel="noreferrer"
                  aria-label={social.label}
                  className="block text-muted transition-colors hover:text-foreground"
                >
                  <svg
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                    aria-hidden="true"
                  >
                    <path d={social.path} />
                  </svg>
                </a>
              </li>
            ))}
          </ul>
        </div>

        <div className="flex flex-wrap gap-12 sm:gap-16">
          {columns.map((column) => (
            <div key={column.heading}>
              <p className="font-mono text-[11px] tracking-[0.18em] uppercase text-muted">
                {column.heading}
              </p>
              <ul className="mt-5 space-y-3 text-[13px]">
                {column.links.map((link) => (
                  <li key={link.href}>
                    {link.href.startsWith("/") ? (
                      <Link
                        href={link.href}
                        className="text-muted transition-colors hover:text-foreground"
                      >
                        {link.label}
                      </Link>
                    ) : (
                      <a
                        href={link.href}
                        target="_blank"
                        rel="noreferrer"
                        className="text-muted transition-colors hover:text-foreground"
                      >
                        {link.label}
                      </a>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </div>

      <div className="mx-auto flex max-w-6xl flex-col gap-2 border-t border-line px-6 py-6 text-[12px] text-muted sm:flex-row sm:justify-between">
        <p>Unaudited alpha. No token, no sale, no mainnet.</p>
        <p>
          Robinhood Chain is a trademark of its owner; Erebus is unaffiliated.
        </p>
      </div>
    </footer>
  );
}
