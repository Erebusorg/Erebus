import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Content policy — Erebus",
  description:
    "What the protocol can and cannot enforce about the traffic it carries, and what an exit operator can do about it.",
};

export default function ContentPolicyPage() {
  return (
    <div className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <header className="border-b border-line pb-10">
        <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
          Content policy
        </p>
        <h1 className="mt-6 text-3xl leading-tight tracking-[-0.02em] sm:text-5xl">
          The network cannot read what it carries
        </h1>
        <p className="mt-5 text-[15px] text-muted">
          That is the design, and it decides what a policy can honestly promise.
        </p>
      </header>

      <div className="mt-12 space-y-10 text-[15px] leading-[1.75] text-foreground/85">
        <section>
          <h2 className="text-xl tracking-tight">What we can enforce</h2>
          <p className="mt-4">
            Entry and relay nodes see ciphertext and a next hop, so no rule
            about content is enforceable there without breaking the protocol.
            Only the exit sees a destination and a payload, and only for the
            request it is handling.
          </p>
          <p className="mt-4">
            An exit operator may therefore restrict which destinations it will
            submit to — a policy about endpoints, not about people. A client
            whose request is refused simply builds another path. Neither we nor
            any operator can retroactively identify who sent something, because
            that information is not retained anywhere: it never exists in one
            place to begin with.
          </p>
        </section>

        <section>
          <h2 className="text-xl tracking-tight">What we ask of users</h2>
          <p className="mt-4">
            Do not use Erebus for anything unlawful where you are, and do not
            use it to attack the network it fronts: flooding a destination with
            requests routed through volunteers&apos; nodes is an attack on the
            volunteers as much as on the target.
          </p>
          <p className="mt-4">
            Privacy is not an excuse for fraud. Hiding your position from the
            public is legitimate; hiding it from an obligation you have already
            entered into is not, and the protocol makes no attempt to help with
            that.
          </p>
        </section>

        <section>
          <h2 className="text-xl tracking-tight">What we ask of operators</h2>
          <p className="mt-4">
            Publish your exit policy, do not log what you do not need, and do
            not attempt to strip layers you are not addressed by; a node that
            tampers with a packet is detected at the exit and, once staking
            exists, is slashed for it.
          </p>
        </section>

        <section>
          <h2 className="text-xl tracking-tight">Reports</h2>
          <p className="mt-4">
            Abuse and vulnerability reports go to the{" "}
            <a
              href="https://github.com/Erebusorg/erebus/issues"
              target="_blank"
              rel="noreferrer"
              className="text-accent"
            >
              issue tracker
            </a>{" "}
            or{" "}
            <a
              href="https://x.com/Erebusorg"
              target="_blank"
              rel="noreferrer"
              className="text-accent"
            >
              @Erebusorg
            </a>
            . For what the protocol does and does not hide, the{" "}
            <Link href="/faq" className="text-accent">
              FAQ
            </Link>{" "}
            is more useful than this page.
          </p>
        </section>
      </div>
    </div>
  );
}
