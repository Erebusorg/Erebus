import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Terms — Erebus",
  description:
    "The terms of using this website and the Erebus software: no warranty, no service, no advice.",
};

export default function TermsPage() {
  return (
    <div className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <header className="border-b border-line pb-10">
        <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
          Terms
        </p>
        <h1 className="mt-6 text-3xl leading-tight tracking-[-0.02em] sm:text-5xl">
          There is nothing here to sign up for
        </h1>
        <p className="mt-5 text-[15px] text-muted">
          Erebus is source code and a website, not a service. These terms are
          short because there is very little to agree to.
        </p>
      </header>

      <div className="mt-12 space-y-10 text-[15px] leading-[1.75] text-foreground/85">
        <section>
          <h2 className="text-xl tracking-tight">The software</h2>
          <p className="mt-4">
            The code is published under the licence in the repository, and that
            licence governs it. It is provided as is, without warranty of any
            kind. It is unaudited alpha software with documented cryptographic
            simplifications, and it may fail to protect you.
          </p>
          <p className="mt-4">
            Running a node or a client is your decision and your responsibility,
            including the legal consequences of relaying other people&apos;s
            traffic in your jurisdiction. Nobody operates the software on your
            behalf, and there is no support obligation.
          </p>
        </section>

        <section>
          <h2 className="text-xl tracking-tight">No advice, no offer</h2>
          <p className="mt-4">
            Nothing on this site is financial, investment, tax, or legal advice.
            There is no token, no sale, no allocation, and no expectation of
            return; anyone offering you one is not us. Mentions of Robinhood
            Chain describe a public network we build toward and do not imply any
            affiliation or endorsement.
          </p>
        </section>

        <section>
          <h2 className="text-xl tracking-tight">The website</h2>
          <p className="mt-4">
            The pages may be wrong or out of date, and they change without
            notice. Where a page and the code disagree, the code is correct.
            What the site collects is described in the{" "}
            <Link href="/privacy" className="text-accent">
              privacy note
            </Link>
            , and what may be carried over the network is described in the{" "}
            <Link href="/content-policy" className="text-accent">
              content policy
            </Link>
            .
          </p>
        </section>

        <section>
          <h2 className="text-xl tracking-tight">Liability</h2>
          <p className="mt-4">
            To the extent the law allows, the authors are not liable for any
            loss arising from use of the software or the site, including lost
            funds, deanonymisation, or damages of any kind.
          </p>
        </section>
      </div>
    </div>
  );
}
