import type { Metadata } from "next";
import { PageHeader } from "@/components/page-header";
import { TextBody, TextSection } from "@/components/text-section";

export const metadata: Metadata = {
  title: "Privacy — Erebus",
  description:
    "What this website collects, which is nothing, and what running the software does on your machine.",
};

export default function PrivacyPage() {
  return (
    <div className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <PageHeader eyebrow="Privacy" title="This site collects nothing">
        A privacy project that ran analytics would be a poor advertisement for
        itself.
      </PageHeader>

      <TextBody>
        <TextSection title="The website">
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
        </TextSection>

        <TextSection title="The software">
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
        </TextSection>

        <TextSection title="Alpha software">
          <p className="mt-4">
            Nothing here has been audited, the cryptography contains documented
            simplifications, and there is no warranty of any kind. Do not route
            anything through it that you would mind exposing.
          </p>
        </TextSection>
      </TextBody>
    </div>
  );
}
