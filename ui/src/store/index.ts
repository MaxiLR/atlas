import type { Activity } from "@/core/generated/Activity";
import type { Citation } from "@/core/generated/Citation";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CoreResponse } from "@/core/generated/CoreResponse";
import type { GenerationOutcome } from "@/core/generated/GenerationOutcome";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";
import type { SideChatSummary } from "@/core/generated/SideChatSummary";
import type { Source } from "@/core/generated/Source";
import type { UsageState } from "@/core/generated/UsageState";
import type { VersionSummary } from "@/core/generated/VersionSummary";

import { initialCourse, type CourseSlice } from "./course";
import { initialGlossary, type GlossarySlice } from "./glossary";
import { initialLibrary, type LibrarySlice } from "./library";
import { initialModels, type ModelsSlice } from "./models";
import { initialNavigation, type NavigationSlice } from "./navigation";
import { initialPlacement, type PlacementSlice } from "./placement";
import { initialSections, type SectionsSlice } from "./sections";
import { initialSession, type SessionSlice } from "./session";
import { initialSideChats, type SideChatsSlice } from "./side-chats";
import type { ShownBlock } from "./stream";
import { initialUsage, type UsageSlice } from "./usage";

/** Everything the interface shows, fed by the core's events and responses. */
export interface AtlasState {
  session: SessionSlice;
  models: ModelsSlice;
  library: LibrarySlice;
  placement: PlacementSlice;
  course: CourseSlice;
  sections: SectionsSlice;
  glossary: GlossarySlice;
  sideChats: SideChatsSlice;
  usage: UsageSlice;
  navigation: NavigationSlice;
}

export const initialState: AtlasState = {
  session: initialSession,
  models: initialModels,
  library: initialLibrary,
  placement: initialPlacement,
  course: initialCourse,
  sections: initialSections,
  glossary: initialGlossary,
  sideChats: initialSideChats,
  usage: initialUsage,
  navigation: initialNavigation,
};

/** The state after one core event. */
export function applyEvent(state: AtlasState, _event: CoreEvent): AtlasState {
  return state;
}

/** The state once `request` has been sent, before the core answers. */
export function applyRequest(
  state: AtlasState,
  _request: CoreRequest,
): AtlasState {
  return state;
}

/** The state after the core answered `request` with `response`. */
export function applyResponse(
  state: AtlasState,
  _request: CoreRequest,
  _response: CoreResponse,
): AtlasState {
  return state;
}

/** The state after the core refused `request`. */
export function applyFailure(
  state: AtlasState,
  _request: CoreRequest,
  _error: CoreError,
): AtlasState {
  return state;
}

/** A section as the reading column shows it. */
export interface SectionDisplay {
  /**
   * `empty` before anything arrived, `streaming` while generated, `stored`
   * for a stored version (or a generation that completed), `incomplete` for
   * a stopped or failed generation.
   */
  status: "empty" | "streaming" | "stored" | "incomplete";
  blocks: ShownBlock[];
  citations: Citation[];
  sources: Source[];
  activity: Activity | null;
  /** How the shown generation ended; null for a stored version. */
  outcome: GenerationOutcome | null;
  /** Why the generation or the last request failed. */
  error: CoreError | null;
  /** The stored version shown; null while a generation is shown. */
  versionId: string | null;
  versions: VersionSummary[];
}

export function selectSection(
  _state: AtlasState,
  _sectionId: string,
): SectionDisplay | null {
  return null;
}

/** The glossary terms and side chats anchored to a section's current version. */
export function selectAnchors(
  _state: AtlasState,
  _sectionId: string,
): { terms: GlossaryEntry[]; chats: SideChatSummary[] } {
  return { terms: [], chats: [] };
}

/** The usage meter: the last reading stays, stale, when a later one fails. */
export function selectUsage(_state: AtlasState): UsageState | null {
  return null;
}
