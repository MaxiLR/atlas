import type { Activity } from "@/core/generated/Activity";
import type { CheckResult } from "@/core/generated/CheckResult";
import type { Citation } from "@/core/generated/Citation";
import type { CoreError } from "@/core/generated/CoreError";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { GenerationOutcome } from "@/core/generated/GenerationOutcome";
import type { SectionVersion } from "@/core/generated/SectionVersion";
import type { SectionView } from "@/core/generated/SectionView";
import type { Source } from "@/core/generated/Source";
import type { VersionStyle } from "@/core/generated/VersionStyle";
import type { VersionSummary } from "@/core/generated/VersionSummary";

import {
  applyGeneration,
  outcomeError,
  shownBlocks,
  type ShownBlock,
  type Stream,
} from "./stream";

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

function entry(
  slice: SectionsSlice,
  courseId: string,
  sectionId: string,
): SectionEntry {
  return (
    slice.byId[sectionId] ?? {
      id: sectionId,
      courseId,
      view: null,
      versions: {},
      currentVersionId: null,
      attempt: null,
      error: null,
    }
  );
}

function put(slice: SectionsSlice, section: SectionEntry): SectionsSlice {
  return { byId: { ...slice.byId, [section.id]: section } };
}

/** One step of a generation writing a version of a section. */
export function sectionGenerated(
  slice: SectionsSlice,
  target: { courseId: string; sectionId: string; style: VersionStyle },
  generationId: string,
  event: GenerationEvent,
): SectionsSlice {
  const section = entry(slice, target.courseId, target.sectionId);
  return put(slice, {
    ...section,
    attempt: {
      style: target.style,
      stream: applyGeneration(
        section.attempt?.stream ?? null,
        generationId,
        event,
      ),
    },
    error: event.type === "Started" ? null : section.error,
  });
}

/**
 * The core answered with a section in its current version. A completed
 * generation gives way to the stored version; a stopped or failed one stays
 * shown unless `dismissAttempt`, as when the candidate chose a version.
 */
export function sectionAnswered(
  slice: SectionsSlice,
  view: SectionView,
  dismissAttempt: boolean,
): SectionsSlice {
  const section = entry(slice, view.courseId, view.id);
  const outcome = section.attempt?.stream.outcome;
  const attemptEnded =
    outcome?.type === "Completed" ||
    (dismissAttempt && outcome !== null && outcome !== undefined);
  return put(slice, {
    ...section,
    view,
    versions: { ...section.versions, [view.version.id]: view.version },
    currentVersionId: view.version.id,
    attempt: attemptEnded ? null : section.attempt,
    error: null,
  });
}

/** A request about a section was refused, or is sent again (`error` null). */
export function sectionFailed(
  slice: SectionsSlice,
  courseId: string,
  sectionId: string,
  error: CoreError | null,
): SectionsSlice {
  const section = entry(slice, courseId, sectionId);
  return section.error === error ? slice : put(slice, { ...section, error });
}

/** A check question of a stored version was answered. */
export function checkAnswered(
  slice: SectionsSlice,
  answer: { versionId: string; block: number; question: number },
  result: CheckResult,
): SectionsSlice {
  const section = Object.values(slice.byId).find(
    (candidate) => answer.versionId in candidate.versions,
  );
  if (section === undefined) return slice;
  const version = section.versions[answer.versionId];
  const blocks = version.blocks.map((block) =>
    block.index === answer.block && block.content.type === "Check"
      ? {
          ...block,
          content: {
            ...block.content,
            questions: block.content.questions.map((question) =>
              question.index === answer.question
                ? { ...question, answer: result }
                : question,
            ),
          },
        }
      : block,
  );
  return put(slice, {
    ...section,
    versions: { ...section.versions, [version.id]: { ...version, blocks } },
  });
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
  /**
   * Why the last request about it was refused, otherwise why the shown
   * generation failed: a refusal is newer than the attempt it left shown.
   */
  error: CoreError | null;
  /** The stored version shown; null while a generation is shown. */
  versionId: string | null;
  /** Every stored version, oldest first. */
  versions: VersionSummary[];
}

const nothing: never[] = [];

/**
 * What a section shows: its generation while one runs or after one stopped
 * or failed, otherwise its current version. Every field is held in the
 * state, so the result compares equal field by field until the state changes.
 */
export function sectionDisplay(
  slice: SectionsSlice,
  sectionId: string,
): SectionDisplay | null {
  const section = slice.byId[sectionId];
  if (section === undefined) return null;
  const versions = section.view?.versions ?? nothing;
  if (section.attempt !== null) {
    const { stream } = section.attempt;
    return {
      status:
        stream.outcome === null
          ? "streaming"
          : stream.outcome.type === "Completed"
            ? "stored"
            : "incomplete",
      blocks: stream.blocks,
      citations: stream.citations,
      sources: stream.sources,
      activity: stream.activity,
      outcome: stream.outcome,
      error: section.error ?? outcomeError(stream.outcome),
      versionId: null,
      versions,
    };
  }
  const version =
    section.currentVersionId === null
      ? undefined
      : section.versions[section.currentVersionId];
  return {
    status: version === undefined ? "empty" : "stored",
    blocks: version === undefined ? nothing : shownBlocks(version.blocks),
    citations: version?.citations ?? nothing,
    sources: version?.sources ?? nothing,
    activity: null,
    outcome: null,
    error: section.error,
    versionId: version?.id ?? null,
    versions,
  };
}
