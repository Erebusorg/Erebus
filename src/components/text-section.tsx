import type { ReactNode } from "react";

/** Body column for the pages that are prose written in JSX rather than markdown. */
export function TextBody({ children }: { children: ReactNode }) {
  return (
    <div className="mt-12 space-y-10 text-[15px] leading-[1.75] text-foreground/85">
      {children}
    </div>
  );
}

export function TextSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section>
      <h2 className="text-xl tracking-tight">{title}</h2>
      {children}
    </section>
  );
}
