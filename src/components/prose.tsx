import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

/** Renders the Markdown sources in `content/` with the site's typography. */
export function Prose({ children }: { children: string }) {
  return (
    <div className="paper">
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={{
          h2: (props) => (
            <h2
              className="mt-16 mb-5 border-b border-line pb-3 text-xl tracking-tight"
              {...props}
            />
          ),
          h3: (props) => (
            <h3
              className="mt-10 mb-3 text-[15px] font-medium tracking-tight text-foreground"
              {...props}
            />
          ),
          p: (props) => (
            <p
              className="my-5 text-[15px] leading-[1.75] text-foreground/85"
              {...props}
            />
          ),
          ul: (props) => (
            <ul
              className="my-5 list-disc space-y-2 pl-5 text-[15px] leading-[1.7] text-foreground/85"
              {...props}
            />
          ),
          ol: (props) => (
            <ol
              className="my-5 list-decimal space-y-2 pl-5 text-[15px] leading-[1.7] text-foreground/85"
              {...props}
            />
          ),
          strong: (props) => (
            <strong className="font-medium text-foreground" {...props} />
          ),
          em: (props) => <em className="text-muted not-italic" {...props} />,
          code: (props) => (
            <code
              className="bg-foreground/5 px-1 py-0.5 font-mono text-[13px] text-foreground"
              {...props}
            />
          ),
          pre: (props) => (
            <pre
              className="my-7 overflow-x-auto border border-line p-5 font-mono text-[12.5px] leading-relaxed text-muted"
              {...props}
            />
          ),
          table: (props) => (
            <div className="my-7 overflow-x-auto">
              <table
                className="w-full border-collapse text-[13px]"
                {...props}
              />
            </div>
          ),
          th: (props) => (
            <th
              className="border-b border-line py-2.5 pr-4 text-left font-mono text-[11px] tracking-[0.14em] uppercase text-muted"
              {...props}
            />
          ),
          td: (props) => (
            <td
              className="border-b border-line py-2.5 pr-4 align-top text-foreground/85"
              {...props}
            />
          ),
          hr: () => <hr className="my-12 border-line" />,
          a: (props) => (
            <a
              className="text-accent underline decoration-accent/40 underline-offset-2"
              {...props}
            />
          ),
        }}
      >
        {children}
      </Markdown>
    </div>
  );
}
