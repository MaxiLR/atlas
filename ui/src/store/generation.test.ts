import { describe, expect, it } from "vitest";

import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { SectionView } from "@/core/generated/SectionView";
import type { VersionStyle } from "@/core/generated/VersionStyle";

import {
  applyEvent,
  applyResponse,
  initialState,
  selectSection,
  type AtlasState,
} from "./index";

const courseId = "course-1";
const sectionId = "section-1";

function generation(
  generationId: string,
  event: GenerationEvent,
  style: VersionStyle = "Original",
): CoreEvent {
  return {
    type: "Generation",
    generationId,
    target: { type: "Section", courseId, sectionId, style },
    event,
  };
}

function replay(state: AtlasState, events: CoreEvent[]): AtlasState {
  return events.reduce(applyEvent, state);
}

const visual = {
  type: "Visual" as const,
  spec: { root: "callout" },
  description: "Plates over the mantle.",
};

/** A recorded stream: prose, a visual that completes late, more prose. */
const recorded: GenerationEvent[] = [
  { type: "Started" },
  { type: "Activity", activity: { type: "Writing" } },
  { type: "BlockOpened", block: 0, kind: "Prose" },
  { type: "TextDelta", block: 0, text: "Plates " },
  { type: "BlockOpened", block: 1, kind: "Visual" },
  { type: "BlockOpened", block: 2, kind: "Prose" },
  { type: "TextDelta", block: 2, text: "Below them " },
  { type: "TextDelta", block: 0, text: "move." },
  { type: "TextDelta", block: 2, text: "is the mantle." },
];

function streamed(generationId = "gen-1"): AtlasState {
  return replay(
    initialState,
    recorded.map((event) => generation(generationId, event)),
  );
}

function storedSection(versionId: string): SectionView {
  return {
    id: sectionId,
    courseId,
    title: "Crust and mantle",
    summary: "What the plates are.",
    version: {
      id: versionId,
      style: "Original",
      blocks: [
        { index: 0, content: { type: "Prose", text: "Plates move." } },
        { index: 1, content: visual },
        {
          index: 2,
          content: { type: "Prose", text: "Below them is the mantle." },
        },
      ],
      citations: [],
      sources: [],
      writtenWithoutSearch: false,
      model: "gpt-test",
      createdAt: "2026-11-19T10:00:00Z",
    },
    versions: [
      { id: versionId, style: "Original", createdAt: "2026-11-19T10:00:00Z" },
    ],
  };
}

describe("a section's generation stream", () => {
  it("yields its blocks in order, appending each delta to its own block", () => {
    const shown = selectSection(streamed(), sectionId);

    expect(shown?.status).toBe("streaming");
    expect(shown?.activity).toEqual({ type: "Writing" });
    expect(shown?.blocks).toEqual([
      { state: "open", index: 0, kind: "Prose", text: "Plates move." },
      { state: "open", index: 1, kind: "Visual", text: "" },
      {
        state: "open",
        index: 2,
        kind: "Prose",
        text: "Below them is the mantle.",
      },
    ]);
  });

  it("replaces a placeholder in place when its block completes", () => {
    const state = replay(streamed(), [
      generation("gen-1", {
        type: "BlockCompleted",
        block: { index: 1, content: visual },
      }),
    ]);

    expect(selectSection(state, sectionId)?.blocks).toEqual([
      { state: "open", index: 0, kind: "Prose", text: "Plates move." },
      { state: "complete", index: 1, content: visual },
      {
        state: "open",
        index: 2,
        kind: "Prose",
        text: "Below them is the mantle.",
      },
    ]);
  });

  it("keeps block order when a later block opens first", () => {
    const state = replay(initialState, [
      generation("gen-1", { type: "Started" }),
      generation("gen-1", { type: "BlockOpened", block: 1, kind: "Prose" }),
      generation("gen-1", { type: "BlockOpened", block: 0, kind: "Figure" }),
    ]);

    expect(
      selectSection(state, sectionId)?.blocks.map((block) => block.index),
    ).toEqual([0, 1]);
  });

  it("collects citations and numbered sources once each", () => {
    const source = { number: 1, url: "https://example.org/", title: "Plates" };
    const state = replay(streamed(), [
      generation("gen-1", {
        type: "CitationAdded",
        citation: { block: 0, start: 0, end: 6, source: 1 },
        source,
      }),
      generation("gen-1", {
        type: "CitationAdded",
        citation: { block: 2, start: 0, end: 5, source: 1 },
        source,
      }),
    ]);

    const shown = selectSection(state, sectionId);
    expect(shown?.citations).toEqual([
      { block: 0, start: 0, end: 6, source: 1 },
      { block: 2, start: 0, end: 5, source: 1 },
    ]);
    expect(shown?.sources).toEqual([source]);
  });
});

describe("how a generation ends", () => {
  it("marks the version stored when it completes", () => {
    const finished = replay(streamed(), [
      generation("gen-1", {
        type: "BlockCompleted",
        block: { index: 0, content: { type: "Prose", text: "Plates move." } },
      }),
      generation("gen-1", { type: "Finished", outcome: { type: "Completed" } }),
    ]);

    const shown = selectSection(finished, sectionId);
    expect(shown?.status).toBe("stored");
    expect(shown?.activity).toBeNull();
    expect(shown?.error).toBeNull();
    expect(shown?.blocks[0]).toEqual({
      state: "complete",
      index: 0,
      content: { type: "Prose", text: "Plates move." },
    });

    const answered = applyResponse(
      finished,
      { type: "OpenSection", courseId, sectionId },
      { type: "Section", section: storedSection("version-1") },
    );
    const stored = selectSection(answered, sectionId);
    expect(stored?.status).toBe("stored");
    expect(stored?.versionId).toBe("version-1");
    expect(stored?.blocks.map((block) => block.state)).toEqual([
      "complete",
      "complete",
      "complete",
    ]);
  });

  it("keeps what arrived when stopped, marked incomplete", () => {
    const state = replay(streamed(), [
      generation("gen-1", { type: "Finished", outcome: { type: "Stopped" } }),
    ]);

    const shown = selectSection(state, sectionId);
    expect(shown?.status).toBe("incomplete");
    expect(shown?.outcome).toEqual({ type: "Stopped" });
    expect(shown?.activity).toBeNull();
    expect(shown?.blocks).toEqual(selectSection(streamed(), sectionId)?.blocks);
  });

  it("keeps what arrived when failed, marked incomplete with the error", () => {
    const error = { type: "PlanLimit" as const, resetsAt: null };
    const state = replay(streamed(), [
      generation("gen-1", {
        type: "Finished",
        outcome: { type: "Failed", error },
      }),
    ]);

    const shown = selectSection(state, sectionId);
    expect(shown?.status).toBe("incomplete");
    expect(shown?.error).toEqual(error);
    expect(shown?.blocks).toEqual(selectSection(streamed(), sectionId)?.blocks);
  });

  it("replaces the incomplete attempt with a retry", () => {
    const failed = replay(streamed("gen-1"), [
      generation("gen-1", {
        type: "Finished",
        outcome: { type: "Failed", error: { type: "Offline" } },
      }),
    ]);

    const retrying = replay(failed, [
      generation("gen-2", { type: "Started" }),
      generation("gen-2", { type: "BlockOpened", block: 0, kind: "Prose" }),
      generation("gen-2", { type: "TextDelta", block: 0, text: "Again." }),
    ]);

    const shown = selectSection(retrying, sectionId);
    expect(shown?.status).toBe("streaming");
    expect(shown?.error).toBeNull();
    expect(shown?.outcome).toBeNull();
    expect(shown?.blocks).toEqual([
      { state: "open", index: 0, kind: "Prose", text: "Again." },
    ]);
  });

  it("starts over when the core retries within one generation", () => {
    const state = replay(streamed("gen-1"), [
      generation("gen-1", {
        type: "Activity",
        activity: { type: "Retrying", attempt: 2 },
      }),
      generation("gen-1", { type: "BlockOpened", block: 0, kind: "Prose" }),
    ]);

    const shown = selectSection(state, sectionId);
    expect(shown?.activity).toEqual({ type: "Retrying", attempt: 2 });
    expect(shown?.blocks).toEqual([
      { state: "open", index: 0, kind: "Prose", text: "" },
    ]);
  });

  it("keeps the current version when a new version fails", () => {
    const stored = applyResponse(
      initialState,
      { type: "OpenSection", courseId, sectionId },
      { type: "Section", section: storedSection("version-1") },
    );
    const failed = replay(stored, [
      generation("gen-2", { type: "Started" }, "Simpler"),
      generation(
        "gen-2",
        { type: "BlockOpened", block: 0, kind: "Prose" },
        "Simpler",
      ),
      generation(
        "gen-2",
        {
          type: "Finished",
          outcome: { type: "Incomplete", reason: "max tokens" },
        },
        "Simpler",
      ),
    ]);

    expect(selectSection(failed, sectionId)?.status).toBe("incomplete");
    expect(failed.sections.byId[sectionId]?.currentVersionId).toBe("version-1");
  });
});

describe("a generation answered before its last events arrive", () => {
  // The shell delivers events and responses on separate channels, so a
  // response can overtake the events the core emitted just before it.
  const lastEvents = [
    generation("gen-1", {
      type: "BlockCompleted",
      block: { index: 0, content: { type: "Prose", text: "Plates move." } },
    }),
    generation("gen-1", { type: "Finished", outcome: { type: "Completed" } }),
  ];

  it("shows the stored version the response carries, and the late events change nothing", () => {
    const answered = applyResponse(
      streamed("gen-1"),
      { type: "OpenSection", courseId, sectionId },
      { type: "Section", section: storedSection("version-1") },
    );
    const late = replay(answered, lastEvents);

    for (const state of [answered, late]) {
      const shown = selectSection(state, sectionId);
      expect(shown?.status).toBe("stored");
      expect(shown?.versionId).toBe("version-1");
      expect(shown?.outcome).toBeNull();
      expect(shown?.blocks.map((block) => block.state)).toEqual([
        "complete",
        "complete",
        "complete",
      ]);
    }
  });

  it("leaves a running new version streaming when a request answers the version already shown", () => {
    const stored = applyResponse(
      initialState,
      { type: "OpenSection", courseId, sectionId },
      { type: "Section", section: storedSection("version-1") },
    );
    const rewriting = replay(stored, [
      generation("gen-2", { type: "Started" }, "Simpler"),
      generation(
        "gen-2",
        { type: "BlockOpened", block: 0, kind: "Prose" },
        "Simpler",
      ),
    ]);

    const reopened = applyResponse(
      rewriting,
      { type: "OpenSection", courseId, sectionId },
      { type: "Section", section: storedSection("version-1") },
    );

    const shown = selectSection(reopened, sectionId);
    expect(shown?.status).toBe("streaming");
    expect(shown?.versionId).toBeNull();
  });
});
