import type { Anchor } from "@/core/generated/Anchor";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";

import type { Stream } from "./stream";

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
