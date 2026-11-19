import { useEffect } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";

/** The course's glossary: each term, its definition and where it was defined. */
export function GlossaryList({ courseId }: { courseId: string }) {
  const { send } = useAtlasStore();
  const entries = useAtlas((state) => state.glossary.byCourse[courseId]);

  useEffect(() => {
    send({ type: "ListGlossary", courseId }).catch(failureInStore);
  }, [send, courseId]);

  if (entries === undefined) {
    return (
      <p role="status" className="px-4 py-3 text-13 text-muted-foreground">
        Loading the glossary…
      </p>
    );
  }
  if (entries.length === 0) {
    return (
      <div className="flex flex-col gap-1 px-4 py-3 text-13">
        <p className="text-body">No terms yet.</p>
        <p className="text-muted-foreground">
          Select a word in a section and choose Define to add one.
        </p>
      </div>
    );
  }
  return (
    <ul aria-label="Glossary" className="flex flex-col">
      {entries.map((entry) => (
        <li
          key={entry.id}
          className="flex flex-col gap-1 border-b border-edge px-4 py-3 last:border-b-0"
        >
          <span className="text-15 font-medium text-heading">{entry.term}</span>
          <span className="text-13 text-body">{entry.definition}</span>
          <span className="text-12 text-muted-foreground">
            {entry.sectionTitle}
          </span>
        </li>
      ))}
    </ul>
  );
}
