import { useId } from "react";

import { useCitationScope } from "@/catalog/components/citation/citation";
import { Sources as SourceList } from "@/components/ai-elements/sources";
import type { Source } from "@/core/generated/Source";

/**
 * The sources a text cites, numbered in order of first citation as its
 * chips are, at its end; nothing when it cites none. Each opens in the
 * system browser. It sits in the `CitationScope` of the text it ends.
 */
export function Sources({ sources }: { sources: Source[] }) {
  const headingId = useId();
  const scope = useCitationScope();
  if (sources.length === 0) return null;
  return (
    <section className="mt-11">
      <h2 id={headingId} className="mb-3 text-20 font-medium text-heading">
        Sources
      </h2>
      <SourceList
        aria-labelledby={headingId}
        sources={sources.map((source) => ({
          ...source,
          id: scope.entryId(source.number),
        }))}
        onOpen={scope.open}
      />
    </section>
  );
}
