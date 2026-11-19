import type { Anchor } from "@/core/generated/Anchor";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";
import type { WikipediaResult } from "@/core/generated/WikipediaResult";

import { applyGeneration, type Stream } from "./stream";

/** A definition being written for an anchor, before it joins the glossary. */
export interface DefinitionDraft {
  courseId: string;
  anchor: Anchor;
  stream: Stream;
}

export interface GlossarySlice {
  /** Each course's entries, in the order the core lists them. */
  byCourse: Record<string, GlossaryEntry[]>;
  /** By anchor key (`anchorKey`). */
  drafts: Record<string, DefinitionDraft>;
}

export const initialGlossary: GlossarySlice = { byCourse: {}, drafts: {} };

/** One key per anchored passage. */
export function anchorKey(anchor: Anchor): string {
  return `${anchor.versionId}:${anchor.block}:${anchor.quote}`;
}

export function glossaryListed(
  slice: GlossarySlice,
  courseId: string,
  entries: GlossaryEntry[],
): GlossarySlice {
  return { ...slice, byCourse: { ...slice.byCourse, [courseId]: entries } };
}

/** A definition joined the glossary (or changed): its draft is done. */
export function entryChanged(
  slice: GlossarySlice,
  entry: GlossaryEntry,
): GlossarySlice {
  const entries = slice.byCourse[entry.courseId] ?? [];
  const at = entries.findIndex((known) => known.id === entry.id);
  const drafts = { ...slice.drafts };
  delete drafts[anchorKey(entry.anchor)];
  return {
    byCourse: {
      ...slice.byCourse,
      [entry.courseId]:
        at === -1 ? [...entries, entry] : entries.with(at, entry),
    },
    drafts,
  };
}

export function wikipediaLookedUp(
  slice: GlossarySlice,
  entryId: string,
  wikipedia: WikipediaResult,
): GlossarySlice {
  const owner = Object.values(slice.byCourse)
    .flat()
    .find((entry) => entry.id === entryId);
  return owner === undefined
    ? slice
    : entryChanged(slice, { ...owner, wikipedia });
}

/** One step of a generation writing a definition. */
export function definitionGenerated(
  slice: GlossarySlice,
  target: { courseId: string; anchor: Anchor },
  generationId: string,
  event: GenerationEvent,
): GlossarySlice {
  const key = anchorKey(target.anchor);
  return {
    ...slice,
    drafts: {
      ...slice.drafts,
      [key]: {
        courseId: target.courseId,
        anchor: target.anchor,
        stream: applyGeneration(
          slice.drafts[key]?.stream ?? null,
          generationId,
          event,
        ),
      },
    },
  };
}
