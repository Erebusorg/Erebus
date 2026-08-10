"use client";

import { useCallback, useSyncExternalStore } from "react";

export type Theme = "dark" | "light";

const EVENT = "erebus:theme";
const STORAGE_KEY = "erebus-theme";

/** Runs before paint, so the first frame is already the stored theme. */
export const themeScript = `(function(){try{var t=localStorage.getItem("${STORAGE_KEY}");if(!t)t=window.matchMedia("(prefers-color-scheme: light)").matches?"light":"dark";document.documentElement.dataset.theme=t}catch(e){}})()`;

function subscribe(onChange: () => void) {
  window.addEventListener(EVENT, onChange);
  return () => window.removeEventListener(EVENT, onChange);
}

function read(): Theme {
  return document.documentElement.dataset.theme === "light" ? "light" : "dark";
}

export function ThemeToggle() {
  const theme = useSyncExternalStore(subscribe, read, () => "dark" as Theme);

  const toggle = useCallback(() => {
    const next: Theme = read() === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Private mode: the theme still applies to this page view.
    }
    window.dispatchEvent(new CustomEvent(EVENT, { detail: next }));
  }, []);

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={`Switch to the ${theme === "dark" ? "light" : "dark"} theme`}
      aria-pressed={theme === "light"}
      className="flex h-9 w-9 items-center justify-center border border-line text-muted transition-colors hover:border-muted hover:text-foreground"
    >
      <svg width="15" height="15" viewBox="0 0 24 24" aria-hidden="true">
        <circle
          cx="12"
          cy="12"
          r="8"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
        />
        <path d="M12 4a8 8 0 0 1 0 16Z" fill="currentColor" />
      </svg>
    </button>
  );
}
