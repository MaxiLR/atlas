import { describe, expect, it } from "vitest";

import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";

import {
  applyEvent,
  applyFailure,
  applyRequest,
  initialState,
  selectSection,
  type AtlasState,
} from "./index";

const courseId = "course-1";
const sectionId = "section-1";

function generation(event: GenerationEvent): CoreEvent {
  return {
    type: "Generation",
    generationId: "gen-1",
    target: { type: "Section", courseId, sectionId, style: "Original" },
    event,
  };
}

/** A generation that wrote one block, then failed offline. */
function failedOffline(): AtlasState {
  return [
    generation({ type: "Started" }),
    generation({ type: "BlockOpened", block: 0, kind: "Prose" }),
    generation({ type: "TextDelta", block: 0, text: "Plates move." }),
    generation({
      type: "Finished",
      outcome: { type: "Failed", error: { type: "Offline" } },
    }),
  ].reduce(applyEvent, initialState);
}

describe("a request about a failed section that the core refuses", () => {
  it.each([
    { type: "RetrySection" as const, courseId, sectionId },
    {
      type: "ExplainDifferently" as const,
      courseId,
      sectionId,
      style: "Simpler" as const,
    },
    { type: "OpenSection" as const, courseId, sectionId },
  ])("shows the refusal of $type over the earlier failure", (request) => {
    const refusal = {
      type: "PlanLimit" as const,
      resetsAt: "2026-11-19T15:00:00Z",
    };

    const state = applyFailure(
      applyRequest(failedOffline(), request),
      request,
      refusal,
    );

    const shown = selectSection(state, sectionId);
    expect(shown?.error).toEqual(refusal);
    expect(shown?.status).toBe("incomplete");
    expect(shown?.blocks).toEqual([
      { state: "open", index: 0, kind: "Prose", text: "Plates move." },
    ]);
  });
});
