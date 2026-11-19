import {
  createContext,
  use,
  useLayoutEffect,
  useState,
  type KeyboardEvent,
  type RefObject,
} from "react";

import type { SideChatSummary } from "@/core/generated/SideChatSummary";
import { cn } from "@/lib/utils";
import type { MarkProps } from "@/reader/section/blocks/blocks";

/** What the marks in the text need from the layer that draws them. */
export interface MarksState {
  /** The defined term whose popover is open. */
  openTermId: string | null;
  /** The side chat open in the side panel. */
  openChatId: string | null;
  openTerm(entryId: string): void;
}

export const MarksContext = createContext<MarksState | null>(null);

/** A defined term's mark: a faint dotted underline that reopens its popover. */
function Term({
  id,
  children,
}: {
  id: string;
  children: MarkProps["children"];
}) {
  const marks = use(MarksContext);
  const open = () => marks?.openTerm(id);
  return (
    <span
      role="button"
      tabIndex={0}
      data-term={id}
      aria-haspopup="dialog"
      aria-expanded={marks?.openTermId === id}
      className="cursor-pointer rounded-[3px] underline decoration-atlas-accent decoration-dotted underline-offset-4 transition-colors outline-none hover:bg-atlas-accent-wash focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid aria-expanded:bg-atlas-accent-wash"
      onClick={() => {
        // The end of a drag that selects text is not an activation.
        if (document.getSelection()?.isCollapsed !== false) open();
      }}
      onKeyDown={(event: KeyboardEvent) => {
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        open();
      }}
    >
      {children}
    </span>
  );
}

/**
 * A mark in the text: a defined term, or the passage a side chat is
 * anchored to, tinted while that chat is open.
 */
export function AnchorMark({ id, kind, children }: MarkProps) {
  const marks = use(MarksContext);
  if (kind === "term") return <Term id={id}>{children}</Term>;
  return (
    <span
      data-chat-passage={id}
      className={cn(
        "rounded-[2px] transition-colors",
        marks?.openChatId === id && "bg-atlas-accent-wash",
      )}
    >
      {children}
    </span>
  );
}

/** The passage a margin mark names: its first words. */
function opening(quote: string): string {
  const words = quote.trim().split(/\s+/);
  return words.length <= 6
    ? words.join(" ")
    : `${words.slice(0, 6).join(" ")}…`;
}

interface Bar {
  id: string;
  top: number;
  height: number;
}

/** Where each side chat's passage runs, from its first line to its last. */
function measure(frame: HTMLElement): Bar[] {
  const origin = frame.getBoundingClientRect();
  const spans = new Map<string, { top: number; bottom: number }>();
  for (const part of frame.querySelectorAll<HTMLElement>(
    "[data-chat-passage]",
  )) {
    const id = part.dataset.chatPassage!;
    const rect = part.getBoundingClientRect();
    const known = spans.get(id);
    spans.set(id, {
      top: Math.min(known?.top ?? rect.top, rect.top),
      bottom: Math.max(known?.bottom ?? rect.bottom, rect.bottom),
    });
  }
  return [...spans].map(([id, { top, bottom }]) => ({
    id,
    top: top - origin.top,
    height: bottom - top,
  }));
}

/**
 * A faint mark in the left margin beside each passage a side chat is
 * anchored to; activating it reopens that chat.
 */
export function MarginMarks({
  frame,
  chats,
  openChatId,
  layout,
  onOpen,
}: {
  frame: RefObject<HTMLElement | null>;
  chats: SideChatSummary[];
  openChatId: string | null;
  /** Changes whenever the text may have moved. */
  layout: unknown;
  onOpen: (chatId: string) => void;
}) {
  const [bars, setBars] = useState<Bar[]>([]);

  useLayoutEffect(() => {
    const root = frame.current;
    if (root === null) return;
    const update = () =>
      setBars((current) => {
        const next = measure(root);
        return JSON.stringify(next) === JSON.stringify(current)
          ? current
          : next;
      });
    update();
    // The text reflows when the column or a figure above changes size. A
    // DOM without ResizeObserver (a test's) keeps the first measure.
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(update);
    observer.observe(root);
    return () => observer.disconnect();
  }, [frame, layout]);

  return bars.map(({ id, top, height }) => {
    const chat = chats.find((each) => each.id === id);
    if (chat === undefined) return null;
    const open = openChatId === id;
    return (
      <button
        key={id}
        type="button"
        data-margin-mark={id}
        aria-label={`Side chat on “${opening(chat.anchor?.quote ?? chat.opening)}”`}
        aria-expanded={open}
        style={{ top, height: Math.max(height, 16) }}
        className="group absolute -left-4 w-3 cursor-pointer rounded-xs outline-none focus-visible:outline-2 focus-visible:outline-focus focus-visible:outline-solid"
        onClick={() => onOpen(id)}
      >
        <span
          aria-hidden
          className="absolute inset-y-0 left-1/2 w-0.5 -translate-x-1/2 rounded-full bg-atlas-accent opacity-55 transition-opacity group-hover:opacity-100 group-aria-expanded:opacity-100"
        />
      </button>
    );
  });
}
