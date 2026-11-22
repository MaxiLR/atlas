import {
  BanIcon,
  HistoryIcon,
  OctagonAlertIcon,
  RotateCwIcon,
  TriangleAlertIcon,
  type LucideIcon,
} from "lucide-react";
import { useEffect, type ComponentProps } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTitle,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Spinner } from "@/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { UsageFailure } from "@/core/generated/UsageFailure";
import type { UsageLevel } from "@/core/generated/UsageLevel";
import type { UsageReading } from "@/core/generated/UsageReading";
import type { UsageState } from "@/core/generated/UsageState";
import type { UsageWindow } from "@/core/generated/UsageWindow";
import { cn } from "@/lib/utils";
import { useNarrow } from "@/reader/narrow";
import { selectUsage } from "@/store";
import { unknownTag } from "@/store/unknown-tag";

import { paceOf, untilReset } from "./pace";
import { useNow } from "./use-now";

/** How often the meter looks at the clock between readings. */
const TICK_MS = 10_000;

const MINUTE = 60_000;

const UNITS: Record<string, string> = {
  m: "minute",
  h: "hour",
  d: "day",
  w: "week",
};

/** `7d` is Weekly; any other `<n><unit>` reads as `5-hour`, `30-day`. */
function windowName(name: string): string {
  if (name === "7d") return "Weekly";
  const match = /^(\d+)([mhdw])$/.exec(name);
  return match ? `${match[1]}-${UNITS[match[2]!]}` : name;
}

interface LevelLook {
  words: string;
  spoken: string;
  Icon: LucideIcon;
  /** The mark's and the fill's colour. */
  ink: string;
  fill: string;
}

/** Each level above normal as a mark, words and a colour. */
const LEVELS: Record<UsageLevel, LevelLook | null> = {
  Normal: null,
  High: {
    words: "High",
    spoken: "high",
    Icon: TriangleAlertIcon,
    ink: "text-amber",
    fill: "bg-amber",
  },
  Critical: {
    words: "Critical",
    spoken: "critical",
    Icon: OctagonAlertIcon,
    ink: "text-red",
    fill: "bg-red",
  },
  Limit: {
    words: "At limit",
    spoken: "at its limit",
    Icon: BanIcon,
    ink: "text-red",
    fill: "bg-red",
  },
};

/** Why a reading failed, as the end of a sentence. */
const FAILURES: Record<UsageFailure, string> = {
  Network: "the network could not be reached",
  Auth: "OpenAI did not accept the plan login",
  Rejected: "OpenAI refused the request",
  Timeout: "OpenAI did not answer in time",
  Cancelled: "the reading was cancelled",
  Unexpected: "OpenAI answered in a way Atlas does not understand",
};

/** A window as the meter shows it at `now`. */
interface Shown {
  key: string;
  name: string;
  /** The share used, as a whole percentage. */
  percent: number;
  /** The share of the window's time passed, 0 to 1; null before it starts. */
  pace: number | null;
  figures: string;
  label: string;
  level: LevelLook | null;
}

function show(window: UsageWindow, now: number): Shown {
  const name = windowName(window.name);
  const percent = Math.round(window.used * 100);
  const pace = paceOf(window, now);
  const until = untilReset(window, now);
  const level = LEVELS[window.level];
  return {
    key: window.name,
    name,
    percent,
    pace,
    figures: `${percent}% · ${until === null ? "not started" : `resets in ${until}`}`,
    label: [
      `${name} window`,
      `${percent}% used`,
      ...(level === null ? [] : [level.spoken]),
      pace === null
        ? "not started"
        : `${Math.round(pace * 100)}% of the window elapsed`,
    ].join(", "),
    level,
  };
}

/** The bar: the share used as its fill and the pace mark as a tick. */
function Bar({
  shown,
  stale,
  className,
  ...props
}: { shown: Shown; stale: boolean } & ComponentProps<"div">) {
  return (
    <div
      role="img"
      aria-label={shown.label}
      className={cn("relative py-1.5", className)}
      {...props}
    >
      <div className="relative h-1 rounded-[2px] bg-edge-strong">
        <div
          className={cn(
            "absolute inset-y-0 left-0 rounded-[2px]",
            shown.level?.fill ?? "bg-body",
            stale && "opacity-50",
          )}
          style={{ width: `${Math.min(100, Math.max(0, shown.percent))}%` }}
        />
        {shown.pace !== null && (
          <div
            className="absolute -inset-y-[3px] w-px -translate-x-1/2 bg-heading ring-1 ring-panel"
            style={{ left: `${shown.pace * 100}%` }}
          />
        )}
      </div>
    </div>
  );
}

function LevelMark({ level }: { level: LevelLook }) {
  return (
    <span className="flex items-center gap-1 font-normal text-body">
      <level.Icon aria-hidden className={cn("size-3", level.ink)} />
      {level.words}
    </span>
  );
}

/** One window: its name and level, its figures, and its bar. */
function WindowRow({ shown, stale }: { shown: Shown; stale: boolean }) {
  return (
    <div>
      <dl className="flex items-baseline justify-between gap-2 text-12 text-muted-foreground">
        <dt className="flex min-w-0 items-center gap-2 font-medium text-body">
          <span className="truncate">{shown.name}</span>
          {shown.level !== null && <LevelMark level={shown.level} />}
        </dt>
        <dd className="shrink-0 tabular-nums">{shown.figures}</dd>
      </dl>
      <Bar shown={shown} stale={stale} />
    </div>
  );
}

function staleSentence(reading: UsageReading, failure: UsageFailure): string {
  const time = new Date(reading.takenAt).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
  return `The latest reading failed: ${FAILURES[failure]}. These figures are from ${time}.`;
}

/** The 5-hour and weekly windows first; the others go in the popover. */
function split(windows: UsageWindow[]) {
  const main = windows.filter(({ name }) => name === "5h" || name === "7d");
  if (main.length === 0) {
    return { main: windows.slice(0, 2), others: windows.slice(2) };
  }
  return { main, others: windows.filter((window) => !main.includes(window)) };
}

/** A reading: the windows as bars, the stale mark and the other windows. */
function Windows({
  reading,
  compact,
}: {
  reading: UsageReading;
  compact: boolean;
}) {
  const wall = useNow(TICK_MS);
  // The figures move in whole minutes counted from the reading, so a fresh
  // reading shows the times it reported rather than a minute less.
  const taken = Date.parse(reading.takenAt);
  const now = taken + Math.max(0, Math.floor((wall - taken) / MINUTE)) * MINUTE;
  const { main, others } = split(reading.windows);
  const stale = reading.stale;
  const staleText = stale === null ? null : staleSentence(reading, stale);

  if (reading.windows.length === 0) {
    return (
      <p className={cn(compact && "sr-only")}>No usage limits reported.</p>
    );
  }

  if (compact) {
    return (
      <div className="flex w-10 flex-col gap-2">
        {main.map((window) => {
          const shown = show(window, now);
          return (
            <Tooltip key={shown.key}>
              <TooltipTrigger asChild>
                <Bar
                  shown={shown}
                  stale={stale !== null}
                  tabIndex={0}
                  className="rounded-xs outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
                />
              </TooltipTrigger>
              <TooltipContent
                side="right"
                className="flex-col items-start gap-0.5"
              >
                <span className="flex items-center gap-2 font-medium">
                  {shown.name}
                  {shown.level !== null && <LevelMark level={shown.level} />}
                </span>
                <span className="tabular-nums">{shown.figures}</span>
                {staleText !== null && (
                  <span className="text-muted-foreground">{staleText}</span>
                )}
              </TooltipContent>
            </Tooltip>
          );
        })}
      </div>
    );
  }

  const count = others.length;
  return (
    <>
      <div className="grid gap-1.5">
        {main.map((window) => (
          <WindowRow
            key={window.name}
            shown={show(window, now)}
            stale={stale !== null}
          />
        ))}
      </div>
      {(staleText !== null || count > 0) && (
        <div className="-mx-2 -mb-1 flex items-center gap-2">
          {staleText !== null && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" className="px-2 text-12 font-normal">
                  <HistoryIcon aria-hidden />
                  Out of date
                </Button>
              </TooltipTrigger>
              <TooltipContent side="top" align="start">
                {staleText}
              </TooltipContent>
            </Tooltip>
          )}
          {count > 0 && (
            <Popover>
              <PopoverTrigger asChild>
                <Button
                  variant="ghost"
                  className="ml-auto px-2 text-12 font-normal"
                >
                  {count === 1 ? "1 more window" : `${count} more windows`}
                </Button>
              </PopoverTrigger>
              <PopoverContent
                side="top"
                align="end"
                aria-label="Other windows"
                className="w-72 gap-3"
              >
                <PopoverTitle className="text-13">Other windows</PopoverTitle>
                <div className="grid gap-1.5">
                  {others.map((window) => (
                    <WindowRow
                      key={window.name}
                      shown={show(window, now)}
                      stale={stale !== null}
                    />
                  ))}
                </div>
              </PopoverContent>
            </Popover>
          )}
        </div>
      )}
    </>
  );
}

/** What the meter shows while it has a plan login to read with. */
function MeterState({
  usage,
  compact,
  onRetry,
}: {
  usage: Exclude<UsageState, { type: "NoPlanLogin" }>;
  compact: boolean;
  onRetry: () => void;
}) {
  switch (usage.type) {
    case "AwaitingReading":
      return compact ? (
        <Spinner aria-label="Reading usage" />
      ) : (
        <p role="status" className="flex items-center gap-2">
          <Spinner aria-hidden role="presentation" />
          Reading usage
        </p>
      );
    case "Connected":
      return <Windows reading={usage.reading} compact={compact} />;
    case "FirstReadFailed": {
      const sentence = `Usage could not be read: ${FAILURES[usage.reason]}.`;
      return compact ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button size="icon" aria-label="Try again" onClick={onRetry}>
              <RotateCwIcon aria-hidden />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">{sentence}</TooltipContent>
        </Tooltip>
      ) : (
        <>
          <p>{sentence}</p>
          <Button className="self-start" onClick={onRetry}>
            <RotateCwIcon aria-hidden />
            Try again
          </Button>
        </>
      );
    }
    default:
      return unknownTag(usage);
  }
}

/**
 * The usage meter at the foot of the rail, read with the plan login. Without
 * one it shows nothing; signed in, it says it is reading until the first
 * reading arrives, then shows the 5-hour and weekly windows as bars, the
 * others in a popover. Folded below 1024 px, it shows the bars only, their
 * figures on hover and focus.
 */
export function UsageMeter({ folded }: { folded: boolean }) {
  const { send } = useAtlasStore();
  const usage = useAtlas(selectUsage);
  const narrow = useNarrow();
  const compact = folded && narrow;

  useEffect(() => {
    send({ type: "GetUsage" }).catch(failureInStore);
  }, [send]);

  if (usage === null || usage.type === "NoPlanLogin") return null;
  return (
    <TooltipProvider>
      <section
        aria-label="Plan usage"
        className={cn(
          "flex shrink-0 flex-col gap-2 border-t border-edge px-5 pt-3 pb-4 text-12 text-muted-foreground",
          compact && "items-center px-0",
        )}
      >
        <MeterState
          usage={usage}
          compact={compact}
          // A failed first reading is read again on the next ask.
          onRetry={() => send({ type: "GetUsage" }).catch(failureInStore)}
        />
      </section>
    </TooltipProvider>
  );
}
