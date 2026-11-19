import {
  useCallback,
  useEffect,
  useEffectEvent,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type RefObject,
} from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
import type { Anchor } from "@/core/generated/Anchor";
import type { Citation } from "@/core/generated/Citation";
import type { Source } from "@/core/generated/Source";
import {
  Blocks,
  type Mark,
  type Passage,
} from "@/reader/section/blocks/blocks";
import type { ShownBlock } from "@/store";
import { anchorKey } from "@/store/glossary";

import { DefinitionBody } from "./definition";
import {
  AnchorMark,
  MarginMarks,
  MarksContext,
  type MarksState,
} from "./marks";
import { sentenceAround } from "./sentence";

/** A selection inside one prose block, as the menu acts on it. */
interface Selected extends Passage {
  range: Range;
  sentence: string;
}

/** The definition popover: a term being defined, or one reopened. */
interface Doubt {
  anchor: Anchor;
  /** Where the selection was, until the defined term is in the text. */
  range: Range | null;
  /** Why the request failed; null while it runs or once written. */
  error: unknown;
  /** The sentence it was asked about; null for a term reopened. */
  sentence: string | null;
}

/** An element-like position floating content can be placed against. */
interface VirtualAnchor {
  getBoundingClientRect(): DOMRect;
  contextElement?: Element;
}

/** Where a range shows: its first line, or all of it; null where unmeasurable. */
function rangeRect(range: Range, firstLine: boolean): DOMRect | null {
  if (typeof range.getBoundingClientRect !== "function") return null;
  const rect = firstLine
    ? (range.getClientRects()[0] ?? range.getBoundingClientRect())
    : range.getBoundingClientRect();
  return rect.width === 0 && rect.height === 0 ? null : rect;
}

/**
 * A position for floating content that `place` measures each time it is
 * asked, falling back to the frame, inside whose scrolling it lies.
 */
function useVirtualAnchor(
  frame: RefObject<HTMLElement | null>,
  place: () => DOMRect | null,
): RefObject<VirtualAnchor> {
  const latest = useRef(place);
  useLayoutEffect(() => {
    latest.current = place;
  });
  return useRef<VirtualAnchor>({
    get contextElement() {
      return frame.current ?? undefined;
    },
    getBoundingClientRect: () =>
      latest.current() ??
      frame.current?.getBoundingClientRect() ??
      new DOMRect(),
  });
}

/**
 * Hands focus to the prose element `range` lay in, or else to block `block`
 * of `frame`: a stop outside the tab order, until focus leaves it.
 */
function focusPassage(frame: HTMLElement, range: Range, block: number) {
  const start = range.startContainer;
  const element = start instanceof Element ? start : start.parentElement;
  const passage =
    (start.isConnected
      ? element?.closest<HTMLElement>("[data-prose-block]")
      : null) ??
    frame.querySelector<HTMLElement>(`[data-prose-block="${block}"]`);
  if (passage == null) return;
  passage.tabIndex = -1;
  passage.addEventListener("blur", () => passage.removeAttribute("tabindex"), {
    once: true,
  });
  passage.focus();
}

const nothing: never[] = [];

/**
 * A section's blocks where a doubt can be settled. Selecting text inside one
 * prose block offers Define and Ask (and the keys D and A). Define opens a
 * popover at the selection with the definition as it streams and the
 * Wikipedia row; the term then keeps a dotted underline that reopens it from
 * the store. Ask starts a side chat anchored to the passage and opens the
 * side panel on it; the passage keeps a margin mark that reopens the chat.
 * Marks show only on the version they belong to.
 */
export function SelectableBlocks({
  courseId,
  versionId,
  blocks,
  citations,
  sources,
}: {
  courseId: string;
  /** The stored version shown; null while a generation is shown. */
  versionId: string | null;
  blocks: ShownBlock[];
  citations: Citation[];
  sources: Source[];
}) {
  const { send, navigate, state } = useAtlasStore();
  const entries = useAtlas((current) => current.glossary.byCourse[courseId]);
  const chats = useAtlas((current) => current.sideChats.byCourse[courseId]);
  const panel = useAtlas((current) => current.navigation.panel);
  const frame = useRef<HTMLDivElement>(null);

  // The marks of earlier sittings come with the course's lists.
  useEffect(() => {
    const current = state.getState();
    if (current.glossary.byCourse[courseId] === undefined) {
      send({ type: "ListGlossary", courseId }).catch(failureInStore);
    }
    if (current.sideChats.listed[courseId] !== true) {
      send({ type: "ListSideChats", courseId }).catch(failureInStore);
    }
  }, [send, state, courseId]);

  const terms = useMemo(
    () =>
      versionId === null
        ? nothing
        : (entries ?? nothing).filter(
            (entry) => entry.anchor.versionId === versionId,
          ),
    [entries, versionId],
  );
  const anchoredChats = useMemo(
    () =>
      versionId === null
        ? nothing
        : (chats ?? nothing).filter(
            (chat) => chat.anchor?.versionId === versionId,
          ),
    [chats, versionId],
  );
  const marks = useMemo<Mark[]>(
    () => [
      ...anchoredChats.map((chat) => ({
        id: chat.id,
        kind: "chat",
        block: chat.anchor!.block,
        quote: chat.anchor!.quote,
      })),
      ...terms.map((entry) => ({
        id: entry.id,
        kind: "term",
        block: entry.anchor.block,
        quote: entry.anchor.quote,
      })),
    ],
    [anchoredChats, terms],
  );

  // -- The selection and its menu ---------------------------------------------

  const [selected, setSelected] = useState<Selected | null>(null);
  const [dismissed, setDismissed] = useState<Selected | null>(null);
  const [pointing, setPointing] = useState(false);
  const [doubt, setDoubt] = useState<Doubt | null>(null);

  const onSelection = useCallback((passage: Passage | null) => {
    const selection = document.getSelection();
    if (passage === null || selection === null || selection.rangeCount === 0) {
      setSelected(null);
      return;
    }
    const range = selection.getRangeAt(0).cloneRange();
    setSelected({ ...passage, range, sentence: sentenceAround(range) });
  }, []);

  // The menu waits for the end of a drag that selects text.
  useEffect(() => {
    const down = (event: PointerEvent) => {
      if (frame.current?.contains(event.target as Node)) setPointing(true);
    };
    const up = () => setPointing(false);
    document.addEventListener("pointerdown", down);
    document.addEventListener("pointerup", up);
    document.addEventListener("pointercancel", up);
    return () => {
      document.removeEventListener("pointerdown", down);
      document.removeEventListener("pointerup", up);
      document.removeEventListener("pointercancel", up);
    };
  }, []);

  const menuOpen =
    selected !== null &&
    versionId !== null &&
    !pointing &&
    dismissed !== selected &&
    doubt === null;

  const menuAnchor = useVirtualAnchor(frame, () =>
    selected === null ? null : rangeRect(selected.range, true),
  );

  // -- Define -------------------------------------------------------------------

  const doubtKey = doubt === null ? null : anchorKey(doubt.anchor);
  const doubtEntry =
    doubtKey === null
      ? undefined
      : entries?.findLast((entry) => anchorKey(entry.anchor) === doubtKey);
  const termOf = (entryId: string | undefined) =>
    entryId === undefined
      ? null
      : frame.current?.querySelector<HTMLElement>(`[data-term="${entryId}"]`);

  const doubtAnchor = useVirtualAnchor(
    frame,
    () =>
      termOf(doubtEntry?.id)?.getBoundingClientRect() ??
      (doubt?.range == null ? null : rangeRect(doubt.range, false)),
  );
  const card = useRef<HTMLDivElement>(null);

  // A written definition is looked up on Wikipedia at once, whether its
  // popover is still open or not, so reopening the term asks for nothing.
  const looking = useRef(new Set<string>());
  const [unreached, setUnreached] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const lookUp = useCallback(
    (entryId: string) => {
      if (looking.current.has(entryId)) return;
      looking.current.add(entryId);
      setUnreached((current) => {
        if (!current.has(entryId)) return current;
        const next = new Set(current);
        next.delete(entryId);
        return next;
      });
      send({ type: "LookupWikipedia", entryId })
        .catch(() => setUnreached((current) => new Set(current).add(entryId)))
        .finally(() => looking.current.delete(entryId));
    },
    [send],
  );

  function requestDefinition(
    anchor: Anchor,
    sentence: string,
    range: Range | null,
  ) {
    setDoubt({ anchor, range, error: null, sentence });
    send({ type: "Define", anchor, sentence }).then(
      ({ entry }) => {
        if (entry.wikipedia === null) lookUp(entry.id);
      },
      (error: unknown) =>
        setDoubt((current) =>
          current?.anchor === anchor ? { ...current, error } : current,
        ),
    );
  }

  function define(from: Selected) {
    if (versionId === null) return;
    const anchor = { versionId, block: from.block, quote: from.text.trim() };
    document.getSelection()?.removeAllRanges();
    const known = terms.findLast(
      (entry) => anchorKey(entry.anchor) === anchorKey(anchor),
    );
    if (known === undefined) {
      requestDefinition(anchor, from.sentence, from.range);
    } else {
      setDoubt({
        anchor: known.anchor,
        range: from.range,
        error: null,
        sentence: null,
      });
    }
  }

  // -- Ask ----------------------------------------------------------------------

  // The side chat whose margin mark takes focus back when the panel closes.
  const returnTo = useRef<string | null>(null);

  // The panel opens at once, in the event that asked for it; the core's
  // answer then points it at the chat.
  function ask(anchor: Anchor) {
    navigate({ type: "OpenPanel", tab: "sideChats" });
    send({ type: "StartSideChat", courseId, anchor }).then(({ chat }) => {
      returnTo.current = chat.id;
    }, failureInStore);
  }

  function askAbout(from: Selected) {
    if (versionId === null) return;
    document.getSelection()?.removeAllRanges();
    ask({ versionId, block: from.block, quote: from.text.trim() });
  }

  function openChat(chatId: string) {
    returnTo.current = chatId;
    if (panel.open && panel.chatId === chatId) {
      navigate({ type: "ClosePanel" });
    } else {
      navigate({ type: "OpenPanel", tab: "sideChats" });
      send({ type: "OpenSideChat", chatId }).catch(failureInStore);
    }
  }

  useEffect(() => {
    if (panel.open || returnTo.current === null) return;
    frame.current
      ?.querySelector<HTMLElement>(`[data-margin-mark="${returnTo.current}"]`)
      ?.focus();
    returnTo.current = null;
  }, [panel.open]);

  // -- Keys -----------------------------------------------------------------------

  const onKey = useEffectEvent((event: KeyboardEvent) => {
    if (selected === null) return;
    if (
      event.defaultPrevented ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey
    ) {
      return;
    }
    const target = event.target;
    if (
      target instanceof Element &&
      target.closest("input, textarea, select, [contenteditable]") !== null
    ) {
      return;
    }
    const key = event.key.toLowerCase();
    if (key === "d") {
      event.preventDefault();
      define(selected);
    } else if (key === "a") {
      event.preventDefault();
      askAbout(selected);
    }
  });
  useEffect(() => {
    if (!menuOpen) return;
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [menuOpen]);

  const openTermId = doubtEntry?.id ?? null;
  const openChatId = panel.open ? panel.chatId : null;
  const marksState = useMemo<MarksState>(
    () => ({
      openTermId,
      openChatId,
      openTerm(entryId) {
        const entry = entries?.find((each) => each.id === entryId);
        if (entry !== undefined) {
          setDoubt({
            anchor: entry.anchor,
            range: null,
            error: null,
            sentence: null,
          });
        }
      },
    }),
    [openTermId, openChatId, entries],
  );

  const busy =
    doubt !== null && doubtEntry === undefined && doubt.error === null;

  return (
    <MarksContext value={marksState}>
      <div ref={frame} className="relative">
        <Blocks
          blocks={blocks}
          versionId={versionId}
          citations={citations}
          sources={sources}
          marks={marks}
          markComponent={AnchorMark}
          onSelection={onSelection}
        />
        <MarginMarks
          frame={frame}
          chats={anchoredChats}
          openChatId={openChatId}
          layout={marks}
          onOpen={openChat}
        />
      </div>

      <Popover
        open={menuOpen}
        onOpenChange={(open) => {
          if (!open) setDismissed(selected);
        }}
      >
        <PopoverAnchor virtualRef={menuAnchor} />
        {/* Unmounted, not animated out, once closed: a closing layer would
            take the next Escape, which belongs to the side panel. */}
        {menuOpen && (
          <PopoverContent
            role="toolbar"
            aria-label="Selection"
            aria-orientation="horizontal"
            side="top"
            align="start"
            sideOffset={6}
            collisionPadding={8}
            className="w-auto flex-row gap-0.5 rounded-lg p-[3px]"
            onOpenAutoFocus={(event) => event.preventDefault()}
            onCloseAutoFocus={(event) => event.preventDefault()}
            onPointerDownOutside={(event) => event.preventDefault()}
            onFocusOutside={(event) => event.preventDefault()}
            // Pressing the menu keeps the selection and the focus where they are.
            onMouseDown={(event) => event.preventDefault()}
          >
            <Button
              variant="ghost"
              className="text-body"
              aria-keyshortcuts="D"
              onClick={() => selected !== null && define(selected)}
            >
              Define
              <Kbd aria-hidden>D</Kbd>
            </Button>
            <Button
              variant="ghost"
              className="text-body"
              aria-keyshortcuts="A"
              onClick={() => selected !== null && askAbout(selected)}
            >
              Ask
              <Kbd aria-hidden>A</Kbd>
            </Button>
          </PopoverContent>
        )}
      </Popover>

      <Popover
        open={doubt !== null}
        onOpenChange={(open) => {
          if (!open) setDoubt(null);
        }}
      >
        <PopoverAnchor virtualRef={doubtAnchor} />
        {/* Unmounted, not animated out, once closed, like the menu. */}
        {doubt !== null && (
          <PopoverContent
            ref={card}
            aria-label={`Definition of ${doubtEntry?.term ?? doubt.anchor.quote}`}
            aria-busy={busy}
            side="bottom"
            align="start"
            sideOffset={8}
            collisionPadding={8}
            className="w-90 gap-0 overflow-hidden p-0"
            onOpenAutoFocus={(event) => {
              event.preventDefault();
              card.current?.focus();
            }}
            // Escape hands focus back to the term in the text, or to the
            // passage while it is not defined yet; a press elsewhere leaves
            // it where it went.
            onEscapeKeyDown={() => {
              const term = termOf(doubtEntry?.id);
              if (term != null) {
                term.focus();
              } else if (doubt.range !== null && frame.current !== null) {
                focusPassage(frame.current, doubt.range, doubt.anchor.block);
              }
            }}
            onCloseAutoFocus={(event) => event.preventDefault()}
          >
            <DefinitionBody
              courseId={courseId}
              anchor={doubt.anchor}
              error={doubt.error}
              unreached={unreached}
              onRetry={() =>
                requestDefinition(
                  doubt.anchor,
                  doubt.sentence ?? "",
                  doubt.range,
                )
              }
              onLookUp={lookUp}
              onAsk={(entry) => {
                setDoubt(null);
                ask(entry.anchor);
              }}
            />
          </PopoverContent>
        )}
      </Popover>
    </MarksContext>
  );
}
