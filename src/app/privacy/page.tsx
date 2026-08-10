import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Privacy — Erebus",
  description:
    "What this website collects, which is nothing, and what running the software does on your machine.",
};

export default function PrivacyPage() {
  return (
    <div className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <header className="border-b border-line pb-10">
        <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
          Privacy
        </p>
        <h1 className="mt-6 text-3xl leading-tight tracking-[-0.02em] sm:text-5xl">
          This site collects nothing
        </h1>
        <p className="mt-5 text-[15px] text-muted">
          A privacy project that ran analytics would be a poor advertisement for
          itself.
        </p>
      </header>

      <div className="mt-12 space-y-10 text-[15px] leading-[1.75] text-foreground/85">
        <section>
          <h2 className="text-xl tracking-tight">The website</h2>
          <p className="mt-4">
            No analytics, no tracking pixels, no cookies, no fonts or scripts
            loaded from third parties, and no account to create. Nothing you
            type into the explorer leaves your browser; it is simulated locally
            and never sent anywhere.
          </p>
          <p className="mt-4">
            Whoever hosts these files necessarily sees the requests for them,
            including your IP address, as any web server does. That is the one
            piece of data this site cannot avoid, and it is not aggregated,
            analysed, or shared by us.
          </p>
        </section>

        <section>
          <h2 className="text-xl tracking-tight">The software</h2>
          <p className="mt-4">
            Running a node or a client is entirely on your own machine and
            reports nothing back. Node keys stay in the files you generate.
            There is no telemetry, no crash reporting, and no update check.
          </p>
          <p className="mt-4">
            Once a public network exists, the nodes you route through will see
            what the paper says they see, and no more. That is a property of the
            protocol rather than a promise on a page, which is the point.
          </p>
        </section>

        <section>
          <h2 className="text-xl tracking-tight">Alpha software</h2>
          <p className="mt-4">
            Nothing here has been audited, the cryptography contains documented
            simplifications, and there is no warranty of any kind. Do not route
            anything through it that you would mind exposing.
          </p>
        </section>
      </div>
    </div>
  );
}
