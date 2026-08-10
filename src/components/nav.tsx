"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

const links = [
  { href: "/#what", label: "What it does" },
  { href: "/#mixing", label: "Mixing" },
  { href: "/#use-cases", label: "Use cases" },
  { href: "/#topology", label: "Network" },
  { href: "/paper", label: "Paper" },
];

export function Nav() {
  const [scrolled, setScrolled] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 24);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <header
      className={`sticky top-0 z-40 transition-colors duration-500 ${
        scrolled
          ? "border-b border-line bg-background/80 backdrop-blur-md"
          : "border-b border-transparent"
      }`}
    >
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-6">
        <Link href="/" className="group flex items-center gap-2.5">
          <Mark />
          <span className="text-sm font-medium tracking-[0.2em] uppercase">
            Erebus
          </span>
        </Link>

        <nav className="hidden items-center gap-8 md:flex">
          {links.map((l) => (
            <Link
              key={l.href}
              href={l.href}
              className="text-[13px] text-muted transition-colors hover:text-foreground"
            >
              {l.label}
            </Link>
          ))}
        </nav>

        <div className="flex items-center gap-3">
          <Link
            href="/paper"
            className="hidden border border-line px-4 py-2 text-[13px] text-muted transition-colors hover:border-muted hover:text-foreground sm:block"
          >
            Read paper
          </Link>
          <button
            type="button"
            aria-label="Toggle navigation"
            aria-expanded={open}
            onClick={() => setOpen((v) => !v)}
            className="p-2 text-muted md:hidden"
          >
            <span className="block h-px w-5 bg-current" />
            <span className="mt-1.5 block h-px w-5 bg-current" />
          </button>
        </div>
      </div>

      {open && (
        <nav className="border-t border-line bg-background px-6 py-4 md:hidden">
          {links.map((l) => (
            <Link
              key={l.href}
              href={l.href}
              onClick={() => setOpen(false)}
              className="block py-2 text-sm text-muted"
            >
              {l.label}
            </Link>
          ))}
        </nav>
      )}
    </header>
  );
}

function Mark() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 256 256"
      aria-hidden="true"
      className="text-foreground"
    >
      <defs>
        <clipPath id="nav-mark-bands">
          <rect x="24" y="24" width="208" height="92" />
          <rect x="24" y="128" width="208" height="34" />
          <rect x="24" y="174" width="208" height="58" />
        </clipPath>
      </defs>
      <path fill="currentColor" clipPath="url(#nav-mark-bands)" d="M118.11 40.56A88 88 0 1 0 215.44 137.89A74 74 0 0 1 118.11 40.56Z" />
    </svg>
  );
}
