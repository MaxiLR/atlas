import type { Activity } from "@/core/generated/Activity";
import type { Block } from "@/core/generated/Block";
import type { BlockContent } from "@/core/generated/BlockContent";
import type { BlockKind } from "@/core/generated/BlockKind";
import type { Citation } from "@/core/generated/Citation";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { GenerationOutcome } from "@/core/generated/GenerationOutcome";
import type { Source } from "@/core/generated/Source";

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

/** The stream with `event` of generation `generationId` applied. */
export function applyGeneration(
  stream: Stream | null,
  _generationId: string,
  _event: GenerationEvent,
): Stream | null {
  return stream;
}

/** Stored blocks as shown blocks; the same array for the same blocks. */
export function shownBlocks(_blocks: Block[]): ShownBlock[] {
  return [];
}
