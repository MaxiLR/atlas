import { useId } from "react";

import type { Source } from "@/core/generated/Source";

/** The sources a section cites, numbered in order of first citation. */
export function Sources({ sources }: { sources: Source[] }) {
  const headingId = useId();
  if (sources.length === 0) return null;
  return (
    <section className="mt-11">
      <h2 id={headingId} className="mb-3 text-20 font-medium text-heading">
        Sources
      </h2>
      <ol
        aria-labelledby={headingId}
        className="grid gap-2 text-13 text-muted-foreground"
      >
        {sources.map((source) => (
          <li key={source.number} className="grid grid-cols-[24px_1fr]">
            <span className="tabular-nums">{source.number}</span>
            <span className="text-body">{source.title}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}
