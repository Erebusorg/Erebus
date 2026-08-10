import fs from "node:fs/promises";
import path from "node:path";

/** Reads a Markdown source from `content/`, dropping any front matter. */
export async function loadMarkdown(name: string) {
  const raw = await fs.readFile(
    path.join(process.cwd(), "content", `${name}.md`),
    "utf8",
  );
  return raw.replace(/^---[\s\S]*?---\n/, "").trim();
}
