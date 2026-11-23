import { describe, expect, it } from "vitest";

import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CourseView } from "@/core/generated/CourseView";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";

import {
  applyEvent,
  applyFailure,
  applyResponse,
  initialState,
  selectOpenCourse,
  type AtlasState,
} from "./index";

const courseId = "course-1";

const course: CourseView = {
  id: courseId,
  topic: "Plate tectonics",
  stage: "Ready",
  chapters: [],
  resumeAt: null,
  glossaryCount: 2,
  sideChatCount: 0,
  placement: null,
};

function entry(term: string): GlossaryEntry {
  return {
    id: `entry-${term}`,
    courseId,
    sectionId: "section-1",
    sectionTitle: "Convergent boundaries",
    term,
    definition: `What ${term} means.`,
    anchor: { versionId: "version-1", block: 0, quote: term },
    wikipedia: null,
    createdAt: "2026-11-19T10:00:00Z",
  };
}

const remove: CoreRequest = {
  type: "DeleteGlossaryEntry",
  entryId: "entry-trench",
};

/** The course open with its glossary of trench and rift listed. */
function listed(): AtlasState {
  const state = applyResponse(
    initialState,
    { type: "OpenCourse", courseId },
    { type: "Course", course },
  );
  return applyResponse(
    state,
    { type: "ListGlossary", courseId },
    { type: "Glossary", courseId, entries: [entry("trench"), entry("rift")] },
  );
}

const terms = (state: AtlasState) =>
  state.glossary.byCourse[courseId]?.map(({ term }) => term);

describe("a glossary entry deleted", () => {
  it("leaves the glossary and its count", () => {
    const state = applyResponse(listed(), remove, { type: "Done" });

    expect(terms(state)).toEqual(["rift"]);
    expect(selectOpenCourse(state)?.glossaryCount).toBe(1);
  });

  it("does not come back with a late Wikipedia lookup, a late change, a late definition or a late listing", () => {
    let state = applyResponse(listed(), remove, { type: "Done" });
    state = applyResponse(
      state,
      { type: "LookupWikipedia", entryId: "entry-trench" },
      {
        type: "Wikipedia",
        entryId: "entry-trench",
        result: { type: "NoArticle" },
      },
    );
    state = applyEvent(state, {
      type: "GlossaryChanged",
      courseId,
      entry: entry("trench"),
    });
    state = applyResponse(
      state,
      {
        type: "Define",
        anchor: entry("trench").anchor,
        sentence: "A trench.",
      },
      { type: "Definition", entry: entry("trench") },
    );
    state = applyResponse(
      state,
      { type: "ListGlossary", courseId },
      { type: "Glossary", courseId, entries: [entry("trench"), entry("rift")] },
    );

    expect(terms(state)).toEqual(["rift"]);
    expect(selectOpenCourse(state)?.glossaryCount).toBe(1);
  });

  it("changes nothing when the core refuses it", () => {
    const before = listed();

    const state = applyFailure(before, remove, {
      type: "Store",
      message: "The database is locked by another process.",
    });

    expect(state).toEqual(before);
  });
});
