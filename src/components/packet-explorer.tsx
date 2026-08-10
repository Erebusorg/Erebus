"use client";

import { useCallback, useEffect, useRef, useState } from "react";

const PACKET_SIZE = 32768;
const MEAN_DELAY_MS = 50;

type Stage = {
  hop: string;
  layer: string;
  sees: string;
  /** Sampled on the client only, so the server render stays deterministic. */
  delayMs: number | null;
  digest: string | null;
};

type Phase = "idle" | "running" | "done";

/** Exponential, the same inverse-transform sampling the client uses. */
function sampleDelay(mean: number) {
  return Math.round(-Math.log(1 - Math.random()) * mean);
}

function digest(length = 24) {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

const HOPS = [
  { hop: "Client", layer: "You", mixes: false },
  { hop: "Entry", layer: "Layer 1", mixes: true },
  { hop: "Relay", layer: "Layer 2", mixes: true },
  { hop: "Exit", layer: "Layer 3", mixes: true },
  { hop: "Service", layer: "Destination", mixes: false },
];

function sees(hop: string, message: string) {
  switch (hop) {
    case "Client":
      return `Your message: "${message}"`;
    case "Entry":
      return "Your address, and the relay to forward to";
    case "Relay":
      return "An entry node and an exit node, neither end of the path";
    case "Exit":
      return "The destination and the payload, but not who sent it";
    default:
      return "The request, and a single-use reply block";
  }
}

function buildStages(message: string, sampled: boolean): Stage[] {
  return HOPS.map(({ hop, layer, mixes }) => ({
    hop,
    layer,
    sees: sees(hop, message),
    delayMs: sampled && mixes ? sampleDelay(MEAN_DELAY_MS) : null,
    digest: sampled ? digest() : null,
  }));
}

export function PacketExplorer() {
  const [message, setMessage] = useState("buy 10 AAPL");
  const [stages, setStages] = useState<Stage[]>(() =>
    buildStages("buy 10 AAPL", false),
  );
  const [reached, setReached] = useState(0);
  const [phase, setPhase] = useState<Phase>("idle");
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);

  useEffect(
    () => () => {
      timers.current.forEach(clearTimeout);
    },
    [],
  );

  const send = useCallback(() => {
    timers.current.forEach(clearTimeout);
    timers.current = [];

    const next = buildStages(message.trim() || "buy 10 AAPL", true);
    const instant =
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    setStages(next);
    setReached(0);
    setPhase("running");

    let elapsed = 0;
    next.forEach((stage, index) => {
      elapsed += instant ? 0 : (stage.delayMs ?? 0) + 260;
      timers.current.push(
        setTimeout(() => {
          setReached(index + 1);
          if (index === next.length - 1) setPhase("done");
        }, elapsed),
      );
    });
  }, [message]);

  const total = stages.reduce((sum, s) => sum + (s.delayMs ?? 0), 0);

  return (
    <div>
      <div className="flex flex-col gap-3 sm:flex-row">
        <label htmlFor="explorer-message" className="sr-only">
          Message to send through the mixnet
        </label>
        <input
          id="explorer-message"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") send();
          }}
          maxLength={64}
          className="flex-1 border border-line bg-transparent px-4 py-3 font-mono text-[13px] text-foreground placeholder:text-muted focus:border-muted focus:outline-none"
          placeholder="buy 10 AAPL"
        />
        <button
          type="button"
          onClick={send}
          className="border border-foreground px-6 py-3 text-[13px] tracking-wide transition-colors hover:bg-foreground hover:text-background"
        >
          {phase === "running" ? "Routing…" : "Send through the mixnet"}
        </button>
      </div>

      <ol className="mt-10">
        {stages.map((stage, index) => {
          const arrived = index < reached;
          return (
            <li
              key={stage.hop}
              className={`border-l border-line pl-6 transition-opacity duration-500 ${
                arrived ? "opacity-100" : "opacity-30"
              }`}
            >
              <div className="relative pb-10">
                <span
                  aria-hidden="true"
                  className={`absolute top-1.5 -left-[29px] block h-2 w-2 rounded-full transition-colors duration-500 ${
                    arrived ? "bg-accent" : "bg-line"
                  }`}
                />
                <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
                  <h3 className="text-[16px]">{stage.hop}</h3>
                  <span className="font-mono text-[11px] tracking-[0.18em] uppercase text-muted">
                    {stage.layer}
                  </span>
                  {stage.delayMs !== null && (
                    <span className="font-mono text-[11px] text-accent">
                      held {stage.delayMs} ms
                    </span>
                  )}
                </div>
                <p className="mt-2 text-[14px] leading-relaxed text-muted">
                  {stage.sees}
                </p>
                <p className="mt-3 truncate font-mono text-[11.5px] text-muted/70">
                  {PACKET_SIZE} bytes{stage.digest ? ` · ${stage.digest}…` : ""}
                </p>
              </div>
            </li>
          );
        })}
      </ol>

      <p
        aria-live="polite"
        className="border-t border-line pt-6 text-[13px] text-muted"
      >
        {phase === "done"
          ? `Delivered after ${total} ms of mixing delay. The service replies through a single-use reply block, over a return path chosen independently of this one.`
          : "Every line carries the same 32 KB, and no two hops see the same bytes."}
      </p>
    </div>
  );
}
