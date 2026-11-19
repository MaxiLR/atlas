import type { CoreError } from "@/core/generated/CoreError";
import type { SectionVersion } from "@/core/generated/SectionVersion";
import type { SectionView } from "@/core/generated/SectionView";
import type { VersionStyle } from "@/core/generated/VersionStyle";

import type { Stream } from "./stream";

/** A generation of a section that has not been stored (yet). */
export interface Attempt {
  style: VersionStyle;
  stream: Stream;
}

export interface SectionEntry {
  id: string;
  courseId: string;
  /** The core's last answer for it: title, summary and the version list. */
  view: SectionView | null;
  /** Every stored version received, by id. */
  versions: Record<string, SectionVersion>;
  currentVersionId: string | null;
  /** The generation running, or the last one when it stopped or failed. */
  attempt: Attempt | null;
  /** Why the last request about it failed. */
  error: CoreError | null;
}

export interface SectionsSlice {
  byId: Record<string, SectionEntry>;
}

export const initialSections: SectionsSlice = { byId: {} };
