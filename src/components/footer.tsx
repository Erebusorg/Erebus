import Link from "next/link";
import { Mark } from "@/components/mark";

const columns = [
  {
    heading: "Project",
    links: [
      { href: "/paper", label: "Paper" },
      { href: "/docs", label: "Docs" },
      { href: "/network", label: "Network" },
      { href: "/faq", label: "FAQ" },
    ],
  },
  {
    heading: "Elsewhere",
    links: [
      { href: "https://github.com/Erebusorg/erebus", label: "GitHub" },
      { href: "https://docs.robinhood.com/chain/", label: "Robinhood Chain" },
    ],
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
        </div>

        <div className="flex gap-16">
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
    </footer>
  );
}
