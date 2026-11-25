import { ArrowRightIcon, ExternalLinkIcon } from "lucide-react";
import { useEffect, useId, useMemo, useRef, useState } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";
import { DeleteMenu, useFocusAfterDelete } from "@/reader/delete-controls";

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
 * One entry: the term, its definition, the section it was defined in and
 * the Wikipedia article, with the menu that deletes it at the top right.
 */
function GlossaryRow({
  entry,
  onDelete,
}: {
  entry: GlossaryEntry;
  onDelete: (entry: GlossaryEntry) => void;
}) {
  const { send } = useAtlasStore();
  const termId = useId();
  return (
    <li className="grid grid-cols-[minmax(0,1fr)_28px] gap-x-2 gap-y-1.5 border-b border-edge pt-3.5 pr-2 pb-2.5 pl-4 last:border-b-0">
      <dl className="col-start-1 flex flex-col gap-1 break-words">
        <dt id={termId} className="text-15 font-medium text-heading">
          {entry.term}
        </dt>
        <dd className="text-13 text-body">{entry.definition}</dd>
      </dl>
      <p className="col-start-1 flex min-w-0 items-center gap-0.5 text-12 text-muted-foreground">
        <span className="shrink-0">Defined in</span>
        <Button
          variant="ghost"
          className="h-6 min-w-0 shrink px-1.5 text-12 has-[>svg:last-child]:pr-1.5"
          title={entry.sectionTitle}
          onClick={() =>
            send({
              type: "OpenSection",
              courseId: entry.courseId,
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
      <DeleteMenu
        label="Glossary entry menu"
        item="Delete definition"
        describedBy={termId}
        rowId={entry.id}
        onDelete={() => onDelete(entry)}
        className="col-start-2 row-start-1 -mt-1"
      />
    </li>
  );
}

/**
 * The course's glossary, newest first: each term, its definition, the
 * section it was defined in, which opens that section, and the term's
 * Wikipedia article when its lookup found one. Definitions made while the
 * list is open join it from the store. An entry deleted from its menu
 * leaves at once; focus moves to the next entry's menu, else the previous
 * one's, else the empty message.
 */
export function GlossaryList({ courseId }: { courseId: string }) {
  const { send } = useAtlasStore();
  const entries = useAtlas((state) => state.glossary.byCourse[courseId]);
  const [attempt, setAttempt] = useState(0);
  const [failed, setFailed] = useState(false);
  const shown = useMemo(
    () => (entries === undefined ? null : newestFirst(entries)),
    [entries],
  );
  const ids = useMemo(() => shown?.map((entry) => entry.id) ?? null, [shown]);
  const list = useRef<HTMLUListElement>(null);
  const empty = useRef<HTMLParagraphElement>(null);
  const afterDelete = useFocusAfterDelete(ids, (id) => {
    if (id === null) {
      empty.current?.focus();
    } else {
      list.current
        ?.querySelector<HTMLElement>(`[data-row-menu="${id}"]`)
        ?.focus();
    }
  });

  useEffect(() => {
    let current = true;
    send({ type: "ListGlossary", courseId }).catch(() => {
      if (current) setFailed(true);
    });
    return () => {
      current = false;
    };
  }, [send, courseId, attempt]);

  if (shown === null) {
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
  if (shown.length === 0) {
    return (
      <p
        ref={empty}
        tabIndex={-1}
        className="px-4 py-3 text-13 text-muted-foreground outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
      >
        No terms yet: the terms you define while reading appear here.
      </p>
    );
  }
  return (
    <ul ref={list} aria-label="Glossary" className="flex flex-col">
      {shown.map((entry) => (
        <GlossaryRow
          key={entry.id}
          entry={entry}
          onDelete={({ id }) => {
            afterDelete.deleting(id);
            send({ type: "DeleteGlossaryEntry", entryId: id }).catch(
              afterDelete.kept,
            );
          }}
        />
      ))}
    </ul>
  );
}
