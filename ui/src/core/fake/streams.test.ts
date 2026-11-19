import { describe, expect, it, vi } from "vitest";

import { selectSection } from "@/store";
import {
  createValve,
  findSection,
  openCourse,
  placeCourse,
  startFake,
} from "@/test/fake-core";

import type { CoreEvent } from "../generated/CoreEvent";
import type { GenerationEvent } from "../generated/GenerationEvent";

/** The steps of every generation writing section `sectionId`, in order. */
function stepsOf(events: CoreEvent[], sectionId: string): GenerationEvent[] {
  return events.flatMap((event) =>
    event.type === "Generation" &&
    event.target.type === "Section" &&
    event.target.sectionId === sectionId
      ? [event.event]
      : [],
  );
}

describe("a section's stream", () => {
  it("opens each block, fills it and completes it in order, ending completed", async () => {
    const { store, events } = startFake("empty-library");
    const course = await placeCourse(store, "Plate tectonics");
    const section = findSection(course, "Convergent boundaries");

    const { section: view } = await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });

    const steps = stepsOf(events, section.id);
    expect(steps[0]).toEqual({ type: "Started" });
    expect(steps.at(-1)).toEqual({
      type: "Finished",
      outcome: { type: "Completed" },
    });
    const opened = steps.flatMap((step) =>
      step.type === "BlockOpened" ? [step.block] : [],
    );
    expect(opened).toEqual(view.version.blocks.map((block) => block.index));
    for (const block of view.version.blocks) {
      const at = (type: GenerationEvent["type"]) =>
        steps.findIndex(
          (step) =>
            step.type === type &&
            ((step.type === "BlockOpened" && step.block === block.index) ||
              (step.type === "BlockCompleted" &&
                step.block.index === block.index)),
        );
      expect(at("BlockOpened")).toBeLessThan(at("BlockCompleted"));
      const text = steps
        .flatMap((step) =>
          step.type === "TextDelta" && step.block === block.index
            ? [step.text]
            : [],
        )
        .join("");
      if (block.content.type === "Prose") {
        expect(text).toBe(block.content.text);
      } else {
        expect(text).toBe("");
      }
    }
    expect(
      steps.flatMap((step) =>
        step.type === "CitationAdded" ? [step.source.number] : [],
      ),
    ).toEqual(view.version.citations.map((citation) => citation.source));
  });

  it("opens a visual that fails validation as a visual and completes it as its text fallback", async () => {
    const { store, events } = startFake("empty-library");
    const course = await placeCourse(store, "Plate tectonics");
    const section = findSection(course, "Convergent boundaries");

    const { section: view } = await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });

    const fallback = view.version.blocks.find(
      (block) => block.content.type === "Fallback",
    )!;
    expect(
      stepsOf(events, section.id).find(
        (step) => step.type === "BlockOpened" && step.block === fallback.index,
      ),
    ).toEqual({ type: "BlockOpened", block: fallback.index, kind: "Visual" });
  });

  it("shows its text in the store growing in order, one step at a time", async () => {
    const valve = createValve();
    const { store, state } = startFake("default", { pace: valve.pace });
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Transform boundaries");

    const opening = store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });
    const shownText = () => {
      const block = selectSection(state(), section.id)?.blocks[0];
      return block?.state === "open"
        ? block.text
        : block?.content.type === "Prose"
          ? block.content.text
          : "";
    };
    const seen: string[] = [];
    while (selectSection(state(), section.id)?.status !== "stored") {
      const before = shownText();
      valve.step();
      await vi.waitFor(() => {
        const status = selectSection(state(), section.id)?.status;
        expect(shownText() !== before || status === "stored").toBe(true);
      });
      seen.push(shownText());
    }
    const { section: view } = await opening;

    const final = view.version.blocks[0]?.content;
    expect(final?.type === "Prose" && final.text).toBe(seen.at(-1));
    expect(seen.length).toBeGreaterThanOrEqual(3);
    for (const [index, text] of seen.entries()) {
      expect(text.startsWith(seen[index - 1] ?? "")).toBe(true);
    }
  });

  it("is shared by two opens of the same section", async () => {
    const { store, events } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Transform boundaries");
    const request = {
      type: "OpenSection" as const,
      courseId: course.id,
      sectionId: section.id,
    };

    const [first, second] = await Promise.all([
      store.send(request),
      store.send(request),
    ]);

    expect(first.section.version.id).toBe(second.section.version.id);
    expect(
      stepsOf(events, section.id).filter((step) => step.type === "Started"),
    ).toHaveLength(1);
  });
});

describe("Stop", () => {
  it("ends a running stream with outcome stopped, keeping what arrived", async () => {
    const valve = createValve();
    const { store, state, events } = startFake("default", {
      pace: valve.pace,
    });
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Mantle convection");
    const opening = store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });
    valve.step();
    await vi.waitFor(() =>
      expect(
        stepsOf(events, section.id).some((step) => step.type === "TextDelta"),
      ).toBe(true),
    );
    const arrived = selectSection(state(), section.id)?.blocks;

    await store.send({
      type: "StopGeneration",
      courseId: course.id,
      sectionId: section.id,
    });
    valve.open();
    await expect(opening).rejects.toBeDefined();

    const steps = stepsOf(events, section.id);
    expect(steps.at(-1)).toEqual({
      type: "Finished",
      outcome: { type: "Stopped" },
    });
    expect(steps.filter((step) => step.type === "TextDelta")).toHaveLength(1);
    expect(selectSection(state(), section.id)).toMatchObject({
      status: "incomplete",
      outcome: { type: "Stopped" },
      blocks: arrived,
    });
  });
});
