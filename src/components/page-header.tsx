import type { ReactNode } from "react";
import { Mark } from "@/components/mark";

type PageHeaderProps = {
  eyebrow: string;
  title: ReactNode;
  children?: ReactNode;
  /** Full-width content below the intro, inside the header's rule. */
  footer?: ReactNode;
  /** The markdown-backed pages carry the mark beside the eyebrow. */
  mark?: boolean;
};

/** The small mono rubric that opens a section inside a page. */
export function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <h2 className="font-mono text-[11px] tracking-[0.24em] uppercase text-muted">
      {children}
    </h2>
  );
}

export function PageHeader({
  eyebrow,
  title,
  children,
  footer,
  mark = false,
}: PageHeaderProps) {
  const label = (
    <p className="font-mono text-[11px] tracking-[0.24em] uppercase text-accent">
      {eyebrow}
    </p>
  );

  return (
    <header className="border-b border-line pb-10">
      {mark ? (
        <div className="flex items-center gap-3">
          <Mark size={22} className="text-accent" />
          {label}
        </div>
      ) : (
        label
      )}
      <h1 className="mt-6 max-w-3xl text-3xl leading-tight tracking-[-0.02em] sm:text-5xl">
        {title}
      </h1>
      {children && (
        <div className="mt-5 max-w-2xl text-[15px] leading-relaxed text-muted">
          {children}
        </div>
      )}
      {footer}
    </header>
  );
}
