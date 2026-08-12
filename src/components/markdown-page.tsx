import type { ReactNode } from "react";
import { PageHeader } from "@/components/page-header";
import { Prose } from "@/components/prose";
import { loadMarkdown } from "@/lib/content";

type MarkdownPageProps = {
  /** File under content/, without the extension. */
  slug: string;
  eyebrow: string;
  title: ReactNode;
  children?: ReactNode;
};

export async function MarkdownPage({
  slug,
  eyebrow,
  title,
  children,
}: MarkdownPageProps) {
  const body = await loadMarkdown(slug);

  return (
    <article className="mx-auto max-w-3xl px-6 py-20 sm:py-28">
      <PageHeader eyebrow={eyebrow} title={title} mark>
        {children}
      </PageHeader>
      <div className="mt-14">
        <Prose>{body}</Prose>
      </div>
    </article>
  );
}
