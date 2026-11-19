import {
  CircleAlertIcon,
  PenLineIcon,
  RotateCwIcon,
  SearchIcon,
} from "lucide-react";
import { useId, useMemo, useState } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import {
  CitationScope,
  useCitationScope,
} from "@/catalog/components/citation/citation";
import { AgentMessage } from "@/components/ai-elements/message";
import { Sources as SourceList } from "@/components/ai-elements/sources";
import { Tool } from "@/components/ai-elements/tool";
import { Button } from "@/components/ui/button";
import { Trace, TraceIcon, TraceLine, TraceTitle } from "@/components/ui/trace";
import { describeError } from "@/core/client";
import type { ChatMessage } from "@/core/generated/ChatMessage";
import type { Citation } from "@/core/generated/Citation";
import type { CoreError } from "@/core/generated/CoreError";
import type { Source } from "@/core/generated/Source";
import { Prose } from "@/reader/section/blocks/prose";
import { ResetsAt, resetTime } from "@/reader/section/status/section-status";
import { selectUsage, type Stream } from "@/store";

const CLOSING = /^[.,;:!?)\]]*/;

/**
 * Citations with each chip after the punctuation that closes its claim, as
 * the side chat sets them, so a sentence reads whole before its source.
 */
function chipsAfterPunctuation(
  citations: Citation[],
  text: string,
): Citation[] {
  return citations.map((citation) => {
    const closing = CLOSING.exec(text.slice(citation.end))![0].length;
    return closing === 0
      ? citation
      : { ...citation, end: citation.end + closing };
  });
}

/** A reply's text: the sections' safe Markdown in Typeset. */
function ReplyText({
  block,
  text,
  streaming,
  citations,
  sources,
}: {
  block: number;
  text: string;
  streaming: boolean;
  citations: Citation[];
  sources: Source[];
}) {
  const placed = useMemo(
    () => chipsAfterPunctuation(citations, text),
    [citations, text],
  );
  return (
    <Prose
      block={block}
      text={text}
      streaming={streaming}
      citations={placed}
      sources={sources}
    />
  );
}

/** The web search a reply ran: while it runs, then with what it found. */
function SearchRow({
  searching,
  count,
}: {
  searching: boolean;
  count: number;
}) {
  return (
    <Tool
      icon={<SearchIcon />}
      title={searching ? "Searching the web…" : "Searched the web"}
      description={
        count === 0
          ? undefined
          : `${count} ${count === 1 ? "source" : "sources"}`
      }
    />
  );
}

/** The sources a reply cites, under it; nothing when it cites none. */
function ReplySources({ sources }: { sources: Source[] }) {
  const headingId = useId();
  const scope = useCitationScope();
  if (sources.length === 0) return null;
  return (
    <section className="mt-1 flex flex-col gap-1.5">
      <h3 id={headingId} className="text-12 font-medium text-muted-foreground">
        Sources
      </h3>
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

const STOPPED = "You stopped this reply before it was finished.";

/** A reply the core holds: complete, or stopped part way. */
export function StoredReply({ message }: { message: ChatMessage }) {
  const { sources } = message;
  return (
    <CitationScope>
      <div className="flex flex-col gap-3">
        {sources.length > 0 && (
          <SearchRow searching={false} count={sources.length} />
        )}
        <AgentMessage
          actions={
            message.state === "Stopped" ? (
              <span className="text-13">{STOPPED}</span>
            ) : undefined
          }
        >
          <ReplyText
            block={0}
            text={message.text}
            streaming={false}
            citations={message.citations}
            sources={sources}
          />
        </AgentMessage>
        <ReplySources sources={sources} />
      </div>
    </CitationScope>
  );
}

/**
 * A reply as it is written: the search row while and after the agent
 * searches, the step it is on until text arrives, then the text. One that
 * failed keeps what arrived, marked incomplete.
 */
export function StreamingReply({ stream }: { stream: Stream }) {
  const { activity, blocks, citations, sources, outcome } = stream;
  const searching = activity?.type === "Searching";
  const [searched, setSearched] = useState(searching);
  if (searching && !searched) setSearched(true);
  const running = outcome === null;
  const texts = blocks.flatMap((block) =>
    block.state === "open"
      ? [{ index: block.index, text: block.text, streaming: true }]
      : block.content.type === "Prose"
        ? [{ index: block.index, text: block.content.text, streaming: false }]
        : [],
  );
  const written = texts.some(({ text }) => text !== "");

  return (
    <CitationScope>
      <div className="flex flex-col gap-3" aria-busy={running}>
        <div role="status" aria-atomic="false">
          <Trace>
            {(searched || sources.length > 0) && (
              <SearchRow searching={searching} count={sources.length} />
            )}
            {running && !searching && !written && (
              <TraceLine status="running">
                <TraceIcon>
                  {activity?.type === "Retrying" ? (
                    <RotateCwIcon />
                  ) : (
                    <PenLineIcon />
                  )}
                </TraceIcon>
                <TraceTitle>
                  {activity?.type === "Retrying"
                    ? `Retrying, attempt ${activity.attempt}…`
                    : "Writing a reply…"}
                </TraceTitle>
              </TraceLine>
            )}
          </Trace>
        </div>
        {written && (
          <AgentMessage
            actions={
              running ? undefined : (
                <span className="text-13">This reply is incomplete.</span>
              )
            }
          >
            {texts.map(({ index, text, streaming }) => (
              <ReplyText
                key={index}
                block={index}
                text={text}
                streaming={streaming}
                citations={citations.filter(({ block }) => block === index)}
                sources={sources}
              />
            ))}
          </AgentMessage>
        )}
        {written && <ReplySources sources={sources} />}
      </div>
    </CitationScope>
  );
}

/** Why a message got no reply, in the candidate's terms. */
function failureText(error: CoreError): string {
  switch (error.type) {
    case "PlanLimit":
      return "The plan's usage limit is reached.";
    case "PlanNotAllowed":
      return "Plan usage is not allowed for this account or workspace, so Atlas cannot answer side chats. Those already answered stay readable.";
    case "Offline":
      return "The app is offline. Send the message again once it is back online.";
    case "FailedAfterRetries":
      return `This reply could not be written: the request failed after ${error.attempts} attempts.`;
    default:
      return describeError(error);
  }
}

/**
 * A message that got no reply: why, and the way on, as sections show it:
 * Manage usage at the plan's limit, Send again unless the plan cannot be
 * used at all.
 */
export function ReplyFailure({
  error,
  onSendAgain,
}: {
  error: CoreError;
  /** Null when there is no message to send again. */
  onSendAgain: (() => void) | null;
}) {
  const { send } = useAtlasStore();
  const usage = useAtlas(selectUsage);
  const resetsAt =
    error.type === "PlanLimit" ? resetTime(error.resetsAt, usage) : null;
  const canSendAgain = onSendAgain !== null && error.type !== "PlanNotAllowed";
  return (
    <div className="flex flex-col items-start gap-3 rounded-lg border border-edge px-4 py-3 text-13">
      <div role="status" className="flex gap-2">
        <CircleAlertIcon
          aria-hidden
          className="mt-[3px] size-3.5 shrink-0 text-muted-foreground"
        />
        <div className="flex flex-col gap-1">
          <p className="text-body">{failureText(error)}</p>
          {resetsAt !== null && <ResetsAt resetsAt={resetsAt} />}
        </div>
      </div>
      {(error.type === "PlanLimit" || canSendAgain) && (
        <div className="flex flex-wrap gap-2">
          {error.type === "PlanLimit" && (
            <Button
              variant="primary"
              onClick={() =>
                send({ type: "OpenManageUsage" }).catch(failureInStore)
              }
            >
              Manage usage
            </Button>
          )}
          {canSendAgain && <Button onClick={onSendAgain}>Send again</Button>}
        </div>
      )}
    </div>
  );
}
