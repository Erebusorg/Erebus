"use client";

import { useEffect, useRef } from "react";

type Packet = {
  path: number[];
  t: number;
  speed: number;
  cover: boolean;
};

const LAYERS = 3;
const PER_LAYER = 4;

/** "#e8e6e3" to "232,230,227", for canvas fill strings. */
function toRgb(hex: string) {
  const value = hex.trim().replace("#", "");
  if (value.length !== 6) return null;
  const n = Number.parseInt(value, 16);
  if (Number.isNaN(n)) return null;
  return `${(n >> 16) & 255},${(n >> 8) & 255},${n & 255}`;
}

export function MixnetBackdrop() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const reduced = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;

    let width = 0;
    let height = 0;
    let frame = 0;
    let ink = "232,230,227";
    let accent = "184,69,43";

    /** Reads the palette off the document so the canvas follows the theme. */
    const readPalette = () => {
      const styles = getComputedStyle(document.documentElement);
      ink = toRgb(styles.getPropertyValue("--foreground")) ?? ink;
      accent = toRgb(styles.getPropertyValue("--accent")) ?? accent;
    };

    const nodes: { x: number; y: number }[][] = [];

    const layout = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const rect = canvas.getBoundingClientRect();
      width = rect.width;
      height = rect.height;
      canvas.width = width * dpr;
      canvas.height = height * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      nodes.length = 0;
      for (let l = 0; l < LAYERS; l++) {
        const col: { x: number; y: number }[] = [];
        for (let n = 0; n < PER_LAYER; n++) {
          col.push({
            x: width * (0.22 + l * 0.28),
            y: height * (0.2 + (n * 0.6) / (PER_LAYER - 1)),
          });
        }
        nodes.push(col);
      }
    };

    const packets: Packet[] = [];
    const spawn = () => {
      packets.push({
        path: Array.from({ length: LAYERS }, () =>
          Math.floor(Math.random() * PER_LAYER),
        ),
        t: 0,
        speed: 0.0016 + Math.random() * 0.0022,
        cover: Math.random() < 0.45,
      });
    };

    const pointAt = (p: Packet) => {
      const stops = [
        { x: 0, y: height * 0.5 },
        ...p.path.map((n, l) => nodes[l][n]),
        { x: width, y: height * 0.5 },
      ];
      const seg = Math.min(
        Math.floor(p.t * (stops.length - 1)),
        stops.length - 2,
      );
      const local = p.t * (stops.length - 1) - seg;
      const a = stops[seg];
      const b = stops[seg + 1];
      return { x: a.x + (b.x - a.x) * local, y: a.y + (b.y - a.y) * local };
    };

    const draw = () => {
      ctx.clearRect(0, 0, width, height);

      ctx.lineWidth = 0.5;
      ctx.strokeStyle = `rgba(${ink},0.05)`;
      for (let l = 0; l < LAYERS - 1; l++) {
        for (const a of nodes[l]) {
          for (const b of nodes[l + 1]) {
            ctx.beginPath();
            ctx.moveTo(a.x, a.y);
            ctx.lineTo(b.x, b.y);
            ctx.stroke();
          }
        }
      }

      nodes.forEach((col, l) =>
        col.forEach((node, n) => {
          const phase = Math.sin(frame * 0.02 + l * 1.7 + n) * 0.5 + 0.5;
          ctx.beginPath();
          ctx.arc(node.x, node.y, 2.2, 0, Math.PI * 2);
          ctx.fillStyle = `rgba(${ink},${0.15 + phase * 0.35})`;
          ctx.fill();
        }),
      );

      for (let i = packets.length - 1; i >= 0; i--) {
        const p = packets[i];
        p.t += p.speed;
        if (p.t >= 1) {
          packets.splice(i, 1);
          continue;
        }
        const { x, y } = pointAt(p);
        ctx.beginPath();
        ctx.arc(x, y, p.cover ? 1.1 : 1.8, 0, Math.PI * 2);
        ctx.fillStyle = p.cover ? `rgba(${ink},0.22)` : `rgba(${accent},0.85)`;
        ctx.fill();
      }

      if (packets.length < 26 && Math.random() < 0.08) spawn();
      frame++;
      raf = requestAnimationFrame(draw);
    };

    readPalette();
    layout();
    let raf = 0;
    if (reduced) {
      draw();
      cancelAnimationFrame(raf);
    } else {
      raf = requestAnimationFrame(draw);
    }

    const onTheme = () => {
      readPalette();
      if (!reduced) return;
      draw();
      cancelAnimationFrame(raf);
    };

    window.addEventListener("resize", layout);
    window.addEventListener("erebus:theme", onTheme);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", layout);
      window.removeEventListener("erebus:theme", onTheme);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 h-full w-full opacity-70"
    />
  );
}
