import { ExternalLinkIcon } from "lucide-react";
import { useEffect } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { describeError } from "@/core/client";
import type { Anchor } from "@/core/generated/Anchor";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";
import type { ShownBlock } from "@/store";
import { anchorKey } from "@/store/glossary";

/** The prose a definition's generation has delivered so far. */
function streamedText(blocks: ShownBlock[]): string {
  return blocks
    .map((block) =>
      block.state === "open"
        ? block.text
        : block.content.type === "Prose"
          ? block.content.text
          : "",
    )
    .join("");
}

/**
 * The definition kept for `anchor`, once it joined the glossary: the latest
 * if it was defined more than once.
 */
export function useDefinition(
  courseId: string,
  anchor: Anchor,
): GlossaryEntry | undefined {
  const key = anchorKey(anchor);
  return useAtlas((state) =>
    state.glossary.byCourse[courseId]?.findLast(
      (entry) => anchorKey(entry.anchor) === key,
    ),
  );
}

/**
 * The Wikipedia row under a definition: the article's summary with a link
 * that opens it in the browser, or that no article matches, or that
 * Wikipedia could not be reached. A term never looked up is looked up when
 * it shows; the result is kept.
 */
function WikipediaRow({
  entry,
  unreached,
  onLookUp,
}: {
  entry: GlossaryEntry;
  /** The last lookup's request failed. */
  unreached: boolean;
  onLookUp: (entryId: string) => void;
}) {
  const { send } = useAtlasStore();
  const pending = entry.wikipedia === null && !unreached;

  useEffect(() => {
    if (pending) onLookUp(entry.id);
  }, [pending, onLookUp, entry.id]);

  const result = entry.wikipedia;
  const failed = unreached || result?.type === "Failed";
  return (
    <div className="flex flex-col gap-1 border-t border-edge px-4 py-3 text-13 text-muted-foreground">
      <div className="flex items-center justify-between gap-3">
        <span className="font-medium text-body">Wikipedia</span>
        {result?.type === "Found" && (
          <a
            href={result.url}
            className="inline-flex items-center gap-1 rounded-xs transition-colors outline-none hover:text-heading focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
            onClick={(event) => {
              event.preventDefault();
              send({ type: "OpenExternal", url: result.url }).catch(
                failureInStore,
              );
            }}
            onAuxClick={(event) => event.preventDefault()}
          >
            Open article
            <ExternalLinkIcon aria-hidden className="size-3.5" />
          </a>
        )}
      </div>
      {failed ? (
        <div className="flex items-center justify-between gap-3">
          <p>Wikipedia could not be reached.</p>
          <Button variant="ghost" onClick={() => onLookUp(entry.id)}>
            Try again
          </Button>
        </div>
      ) : result === null ? (
        <p role="status">Looking up the article…</p>
      ) : result.type === "Found" ? (
        <p className="line-clamp-5">{result.extract}</p>
      ) : (
        <p>No Wikipedia article matches this term.</p>
      )}
    </div>
  );
}

/**
 * What the definition popover shows: the term, then the definition as it
 * streams in, or why it could not be written; once written, the Wikipedia
 * row and a way to ask about it.
 */
export function DefinitionBody({
  courseId,
  anchor,
  error,
  unreached,
  onRetry,
  onLookUp,
  onAsk,
}: {
  courseId: string;
  anchor: Anchor;
  /** Why the request for it failed; null while it runs or once written. */
  error: unknown;
  /** The entries whose Wikipedia lookup could not be sent. */
  unreached: ReadonlySet<string>;
  onRetry: () => void;
  onLookUp: (entryId: string) => void;
  onAsk: (entry: GlossaryEntry) => void;
}) {
  const entry = useDefinition(courseId, anchor);
  const draft = useAtlas((state) => state.glossary.drafts[anchorKey(anchor)]);
  const streamed = draft === undefined ? "" : streamedText(draft.stream.blocks);
  const retrying =
    draft?.stream.activity?.type === "Retrying"
      ? draft.stream.activity.attempt
      : null;

  return (
    <>
      <div className="flex flex-col gap-1.5 px-4 pt-3.5 pb-3.5">
        <h2 className="text-15 font-medium text-heading">
          {entry?.term ?? anchor.quote}
        </h2>
        {entry !== undefined ? (
          <p className="text-13 text-body">{entry.definition}</p>
        ) : error !== null ? (
          <div className="flex flex-col items-start gap-2 text-13">
            <p className="text-body">The definition could not be written.</p>
            <p className="text-muted-foreground">{describeError(error)}</p>
            <Button onClick={onRetry}>Try again</Button>
          </div>
        ) : streamed !== "" ? (
          <p className="text-13 text-body">{streamed}</p>
        ) : (
          <div className="flex flex-col gap-2">
            <p role="status" className="text-13 text-muted-foreground">
              {retrying === null
                ? "Writing the definition…"
                : `Writing the definition, attempt ${retrying}…`}
            </p>
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-2/3" />
          </div>
        )}
      </div>
      {entry !== undefined && (
        <>
          <WikipediaRow
            entry={entry}
            unreached={unreached.has(entry.id)}
            onLookUp={onLookUp}
          />
          <div className="flex items-center justify-between gap-3 border-t border-edge bg-wash py-2 pr-2 pl-4 text-13 text-muted-foreground">
            In the glossary
            <Button onClick={() => onAsk(entry)}>Ask about this</Button>
          </div>
        </>
      )}
    </>
  );
}
