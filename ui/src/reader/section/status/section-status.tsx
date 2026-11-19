import {
  PenLineIcon,
  RotateCwIcon,
  SearchIcon,
  SearchXIcon,
  SquareIcon,
} from "lucide-react";
import { useEffect, useRef, useState, type MouseEvent } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Tool } from "@/components/ai-elements/tool";
import { Button } from "@/components/ui/button";
import { Trace, TraceIcon, TraceLine, TraceTitle } from "@/components/ui/trace";
import { describeError } from "@/core/client";
import type { Activity } from "@/core/generated/Activity";
import type { UsageState } from "@/core/generated/UsageState";
import { selectSection, selectUsage, type SectionDisplay } from "@/store";

const MINUTE = 60_000;

/** The agent's current step, as the row under a web search shows it. */
function Step({ activity }: { activity: Activity | null }) {
  const [icon, title] =
    activity?.type === "Retrying"
      ? [<RotateCwIcon />, `Retrying, attempt ${activity.attempt}…`]
      : [<PenLineIcon />, "Writing this section…"];
  return (
    <TraceLine status="running">
      <TraceIcon>{icon}</TraceIcon>
      <TraceTitle>{title}</TraceTitle>
    </TraceLine>
  );
}

/**
 * What the agent is doing as it happens, as AI Elements rows: the web search
 * (Tool) once one ran, then the step it is on. The search row stays for the
 * rest of the generation, so the text below does not move when it ends.
 */
function Generating({
  display,
  onStop,
}: {
  display: SectionDisplay;
  onStop: (event: MouseEvent<HTMLButtonElement>) => void;
}) {
  const { activity, sources } = display;
  const searching = activity?.type === "Searching";
  const [searched, setSearched] = useState(searching);
  if (searching && !searched) setSearched(true);
  const count = sources.length;

  return (
    <div className="mb-7 flex items-start justify-between gap-4">
      <div role="status" aria-atomic="false" className="min-w-0 flex-1">
        <Trace>
          {(searched || count > 0) && (
            <Tool
              icon={<SearchIcon />}
              title={searching ? "Searching the web…" : "Searched the web"}
              description={
                count === 0
                  ? undefined
                  : `${count} ${count === 1 ? "source" : "sources"}`
              }
            />
          )}
          {!searching && <Step activity={activity} />}
        </Trace>
      </div>
      <Button variant="ghost" className="-my-1.5" onClick={onStop}>
        <SquareIcon aria-hidden />
        Stop
      </Button>
    </div>
  );
}

/**
 * When the plan can be used again: the time the refusal names, otherwise
 * the latest reset of the usage meter's windows at their limit.
 */
function resetTime(
  refusal: string | null,
  usage: UsageState | null,
): string | null {
  if (refusal !== null) return refusal;
  if (usage?.type !== "Connected") return null;
  const resets = usage.reading.windows
    .flatMap((window) =>
      window.level === "Limit" && window.resetsAt !== null
        ? [window.resetsAt]
        : [],
    )
    .sort((a, b) => Date.parse(a) - Date.parse(b));
  return resets.at(-1) ?? null;
}

const dayAndTime = new Intl.DateTimeFormat("en-GB", {
  weekday: "short",
  hour: "2-digit",
  minute: "2-digit",
});

/**
 * "It resets in 2 h 14 min.", or the day and time when more than a day
 * away; kept current while shown.
 */
function ResetsAt({ resetsAt }: { resetsAt: string }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(timer);
  }, []);
  const minutes = Math.round((Date.parse(resetsAt) - now) / MINUTE);
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  const when =
    minutes <= 0
      ? null
      : minutes < 60
        ? `in ${minutes} min`
        : hours < 24
          ? `in ${hours} h${rest === 0 ? "" : ` ${rest} min`}`
          : dayAndTime.format(new Date(resetsAt));
  return (
    <p className="text-muted-foreground tabular-nums">
      {when === null ? "It has reset." : `It resets ${when}.`}
    </p>
  );
}

/** What went wrong, in the candidate's terms. */
function failureText(display: SectionDisplay): string {
  const { error, outcome } = display;
  switch (error?.type) {
    case "PlanLimit":
      return "The plan's usage limit is reached.";
    case "PlanNotAllowed":
      return "Plan usage is not allowed for this account or workspace, so Atlas cannot write new sections. Sections already written stay readable.";
    case "Offline":
      return "The app is offline. This section is written once it is back online.";
    case "FailedAfterRetries":
      return `This section could not be written: the request failed after ${error.attempts} attempts.`;
  }
  if (outcome?.type === "Stopped") {
    return "You stopped it before it was finished.";
  }
  if (outcome?.type === "Incomplete") {
    return "The response ended before the section was finished.";
  }
  return describeError(error);
}

/**
 * Where the section stands: opening; being written, with what the agent is
 * doing and Stop; stopped or failed, with what arrived marked incomplete and
 * the way on (Retry, Manage usage, or nothing when the plan cannot be used).
 * A stored version written without web search says so in one line.
 */
export function SectionStatus({
  courseId,
  sectionId,
}: {
  courseId: string;
  sectionId: string;
}) {
  const { send } = useAtlasStore();
  const display = useAtlas((state) => selectSection(state, sectionId));
  const versionId = display?.versionId ?? null;
  const withoutSearch = useAtlas(
    (state) =>
      versionId !== null &&
      state.sections.byId[sectionId]?.versions[versionId]
        ?.writtenWithoutSearch === true,
  );
  const usage = useAtlas(selectUsage);
  const retryRef = useRef<HTMLButtonElement>(null);
  // Stop leaves with the generation; Retry takes its focus.
  const focusRetry = useRef(false);
  const status = display?.status;
  useEffect(() => {
    if (status !== "incomplete" || !focusRetry.current) return;
    focusRetry.current = false;
    retryRef.current?.focus();
  }, [status]);

  const ids = { courseId, sectionId };
  const stop = (event: MouseEvent<HTMLButtonElement>) => {
    focusRetry.current = document.activeElement === event.currentTarget;
    send({ type: "StopGeneration", ...ids }).catch(failureInStore);
  };
  const retry = () =>
    send({ type: "RetrySection", ...ids }).catch(failureInStore);

  if (
    display === null ||
    (display.status === "empty" && display.error === null)
  ) {
    return (
      <p role="status" className="mb-7 text-13 text-muted-foreground">
        Opening this section…
      </p>
    );
  }
  if (display.status === "streaming") {
    return <Generating display={display} onStop={stop} />;
  }
  if (display.status === "incomplete" || display.error !== null) {
    const { error } = display;
    const resetsAt =
      error?.type === "PlanLimit" ? resetTime(error.resetsAt, usage) : null;
    return (
      <div className="mb-7 flex flex-col items-start gap-3 rounded-lg border border-edge px-4 py-3 text-13">
        <div role="status" className="flex flex-col gap-1">
          {display.status === "incomplete" && display.blocks.length > 0 && (
            <p className="font-medium text-heading">
              This section is incomplete.
            </p>
          )}
          <p className="text-body">{failureText(display)}</p>
          {resetsAt !== null && <ResetsAt resetsAt={resetsAt} />}
        </div>
        {error?.type !== "PlanNotAllowed" && (
          <div className="flex flex-wrap gap-2">
            {error?.type === "PlanLimit" && (
              <Button
                variant="primary"
                onClick={() =>
                  send({ type: "OpenManageUsage" }).catch(failureInStore)
                }
              >
                Manage usage
              </Button>
            )}
            <Button ref={retryRef} onClick={retry}>
              Retry
            </Button>
          </div>
        )}
      </div>
    );
  }
  if (withoutSearch) {
    return (
      <p className="mb-7 flex items-center gap-2 text-13 text-muted-foreground">
        <SearchXIcon aria-hidden className="size-3.5 shrink-0" />
        Written without web search: the account's policy does not allow it.
      </p>
    );
  }
  return null;
}
