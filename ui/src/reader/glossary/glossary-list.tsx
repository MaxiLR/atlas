import { ArrowRightIcon, ExternalLinkIcon } from "lucide-react";
import { useEffect, useState } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";

/**
 * Newest first. The core lists entries in the order they were defined, so
 * entries defined at the same instant keep the later one on top.
 */
function newestFirst(entries: GlossaryEntry[]): GlossaryEntry[] {
  return entries
    .toReversed()
    .sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt));
}

/**
 * A link to `term`'s Wikipedia article that opens it in the browser through
 * the core.
 */
function ArticleLink({ term, url }: { term: string; url: string }) {
  const { send } = useAtlasStore();
  return (
    <Button
      asChild
      variant="ghost"
      className="h-6 px-1.5 text-12 has-[>svg:last-child]:pr-1.5"
    >
      <a
        href={url}
        aria-label={`Wikipedia article on ${term}`}
        onClick={(event) => {
          event.preventDefault();
          send({ type: "OpenExternal", url }).catch(failureInStore);
        }}
        onAuxClick={(event) => event.preventDefault()}
      >
        Wikipedia
        <ExternalLinkIcon aria-hidden className="size-3" />
      </a>
    </Button>
  );
}

/**
 * The course's glossary, newest first: each term, its definition, the
 * section it was defined in, which opens that section, and the term's
 * Wikipedia article when its lookup found one. Definitions made while the
 * list is open join it from the store.
 */
export function GlossaryList({ courseId }: { courseId: string }) {
  const { send } = useAtlasStore();
  const entries = useAtlas((state) => state.glossary.byCourse[courseId]);
  const [attempt, setAttempt] = useState(0);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let current = true;
    send({ type: "ListGlossary", courseId }).catch(() => {
      if (current) setFailed(true);
    });
    return () => {
      current = false;
    };
  }, [send, courseId, attempt]);

  if (entries === undefined) {
    if (failed) {
      return (
        <div className="flex flex-col items-start gap-2 px-4 py-3 text-13">
          <p className="text-body">The glossary could not load.</p>
          <Button
            onClick={() => {
              setFailed(false);
              setAttempt((count) => count + 1);
            }}
          >
            Try again
          </Button>
        </div>
      );
    }
    return (
      <p role="status" className="px-4 py-3 text-13 text-muted-foreground">
        Loading the glossary…
      </p>
    );
  }
  if (entries.length === 0) {
    return (
      <p className="px-4 py-3 text-13 text-muted-foreground">
        No terms yet: the terms you define while reading appear here.
      </p>
    );
  }
  return (
    <ul aria-label="Glossary" className="flex flex-col">
      {newestFirst(entries).map((entry) => (
        <li
          key={entry.id}
          className="flex flex-col gap-1.5 border-b border-edge px-4 pt-3.5 pb-2.5 last:border-b-0"
        >
          <dl className="flex flex-col gap-1 break-words">
            <dt className="text-15 font-medium text-heading">{entry.term}</dt>
            <dd className="text-13 text-body">{entry.definition}</dd>
          </dl>
          <p className="flex min-w-0 items-center gap-0.5 text-12 text-muted-foreground">
            <span className="shrink-0">Defined in</span>
            <Button
              variant="ghost"
              className="h-6 min-w-0 shrink px-1.5 text-12 has-[>svg:last-child]:pr-1.5"
              title={entry.sectionTitle}
              onClick={() =>
                send({
                  type: "OpenSection",
                  courseId,
                  sectionId: entry.sectionId,
                }).catch(failureInStore)
              }
            >
              <span className="truncate">{entry.sectionTitle}</span>
              <ArrowRightIcon aria-hidden className="size-3" />
            </Button>
            {entry.wikipedia?.type === "Found" && (
              <>
                <span aria-hidden className="mx-0.5 shrink-0">
                  ·
                </span>
                <ArticleLink term={entry.term} url={entry.wikipedia.url} />
              </>
            )}
          </p>
        </li>
      ))}
    </ul>
  );
}
