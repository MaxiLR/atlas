import type { Activity } from "@/core/generated/Activity";
import type { Block } from "@/core/generated/Block";
import type { BlockContent } from "@/core/generated/BlockContent";
import type { BlockKind } from "@/core/generated/BlockKind";
import type { Citation } from "@/core/generated/Citation";
import type { CoreError } from "@/core/generated/CoreError";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { GenerationOutcome } from "@/core/generated/GenerationOutcome";
import type { Source } from "@/core/generated/Source";

import { unknownTag } from "./unknown-tag";

/**
 * A block as shown: a placeholder holding its place (with the text streamed
 * into it so far) until the core completes it.
 */
export type ShownBlock =
  | { state: "open"; index: number; kind: BlockKind; text: string }
  | { state: "complete"; index: number; content: BlockContent };

/**
 * What one generation has delivered so far: a section version, a definition
 * or a side-chat reply. Sections, Define and side chats share it, as they
 * share the core's one generation stream.
 */
export interface Stream {
  generationId: string;
  /** What the agent is doing; null once finished. */
  activity: Activity | null;
  /** In block order, whatever order they arrived in. */
  blocks: ShownBlock[];
  citations: Citation[];
  sources: Source[];
  /** Null while it runs. */
  outcome: GenerationOutcome | null;
}

function started(generationId: string): Stream {
  return {
    generationId,
    activity: null,
    blocks: [],
    citations: [],
    sources: [],
    outcome: null,
  };
}

/** `blocks` with `block` in its place: over the block it replaces, or in index order. */
function placed(blocks: ShownBlock[], block: ShownBlock): ShownBlock[] {
  const at = blocks.findIndex((shown) => shown.index >= block.index);
  if (at === -1) return [...blocks, block];
  return blocks[at].index === block.index
    ? blocks.with(at, block)
    : blocks.toSpliced(at, 0, block);
}

/**
 * The stream with `event` of generation `generationId` applied. Another
 * generation replaces what an earlier one delivered, as does the core
 * starting over within one generation.
 */
export function applyGeneration(
  stream: Stream | null,
  generationId: string,
  event: GenerationEvent,
): Stream {
  const current =
    stream !== null && stream.generationId === generationId
      ? stream
      : started(generationId);
  switch (event.type) {
    case "Started":
      return started(generationId);
    case "Activity":
      return event.activity.type === "Retrying"
        ? { ...started(generationId), activity: event.activity }
        : { ...current, activity: event.activity };
    case "BlockOpened":
      return {
        ...current,
        blocks: placed(current.blocks, {
          state: "open",
          index: event.block,
          kind: event.kind,
          text: "",
        }),
      };
    case "TextDelta": {
      const at = current.blocks.findIndex(
        (block) => block.index === event.block,
      );
      const block = current.blocks[at];
      if (block?.state !== "open") return current;
      return {
        ...current,
        blocks: current.blocks.with(at, {
          ...block,
          text: block.text + event.text,
        }),
      };
    }
    case "BlockCompleted":
      return {
        ...current,
        blocks: placed(current.blocks, {
          state: "complete",
          index: event.block.index,
          content: event.block.content,
        }),
      };
    case "CitationAdded":
      return {
        ...current,
        citations: [...current.citations, event.citation],
        sources: current.sources.some(
          (source) => source.number === event.source.number,
        )
          ? current.sources
          : [...current.sources, event.source],
      };
    case "Finished":
      return { ...current, activity: null, outcome: event.outcome };
    default:
      return unknownTag(event);
  }
}

/** Why a generation failed; null while it runs or when it did not fail. */
export function outcomeError(
  outcome: GenerationOutcome | null,
): CoreError | null {
  if (outcome === null) return null;
  switch (outcome.type) {
    case "Failed":
      return outcome.error;
    case "Completed":
    case "Stopped":
    case "Incomplete":
      return null;
    default:
      return unknownTag(outcome);
  }
}

const shownCache = new WeakMap<Block[], ShownBlock[]>();

/**
 * Stored blocks as shown blocks: the same array for the same blocks, so a
 * selector over a stored version returns a stable value.
 */
export function shownBlocks(blocks: Block[]): ShownBlock[] {
  let shown = shownCache.get(blocks);
  if (shown === undefined) {
    shown = blocks.map((block) => ({
      state: "complete",
      index: block.index,
      content: block.content,
    }));
    shownCache.set(blocks, shown);
  }
  return shown;
}
