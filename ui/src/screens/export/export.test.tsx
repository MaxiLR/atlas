import { render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { App } from "@/App";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import { startedAt } from "@/test/fake-core";

/**
 * jsdom lays nothing out, and Mermaid measures its labels while it draws.
 * These stand in for the browser's measurements: about 7 px a character,
 * 14 px a line.
 */
beforeAll(() => {
  const measured = window.SVGElement.prototype as unknown as {
    getBBox(): DOMRect;
    getComputedTextLength(): number;
  };
  measured.getBBox = function (this: SVGElement) {
    const length = (this.textContent ?? "").length;
    return new DOMRect(0, 0, length * 7, 14);
  };
  measured.getComputedTextLength = function (this: SVGElement) {
    return (this.textContent ?? "").length * 7;
  };
});

/** Mermaid loads on first use; the first diagram takes longest. */
vi.setConfig({ testTimeout: 20_000 });

const READY_EVENT = "atlas:export-state";

afterEach(() => {
  window.history.replaceState(null, "", "/");
  Reflect.deleteProperty(document, "fonts");
});

/** The index of Plate tectonics, in index order. */
const contents = [
  "The solid Earth",
  "Layers by composition",
  "Lithosphere and asthenosphere",
  "Plates and their boundaries",
  "Divergent boundaries",
  "Convergent boundaries",
  "Subduction, step by step",
  "Why the denser plate sinks",
  "Continental collision",
  "Transform boundaries",
  "What moves the plates",
  "Mantle convection",
  "Slab pull and ridge push",
];

/** The sections with a stored version once the first deep dive is read. */
const opened = [
  "Layers by composition",
  "Lithosphere and asthenosphere",
  "Divergent boundaries",
  "Convergent boundaries",
  "Subduction, step by step",
];

const unopened = [
  "Why the denser plate sinks",
  "Continental collision",
  "Transform boundaries",
  "Mantle convection",
  "Slab pull and ridge push",
];

const DIAGRAM = "From the trench to the volcanic arc, step by step.";

/**
 * The app at the export route of Plate tectonics on the fake core, after the
 * candidate read Subduction, step by step, the first deep dive of Convergent
 * boundaries. `courseId` replaces the course's id in the route.
 */
async function renderExport(courseId?: string) {
  const core = createFakeCore({
    scenario: "default",
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  const course = core.world.courses.find(
    (each) => each.topic === "Plate tectonics",
  );
  const convergent = course?.chapters
    .flatMap((chapter) => chapter.sections)
    .find((section) => section.sample.title === "Convergent boundaries");
  const deepDive = convergent?.deepDives?.[0];
  if (course === undefined || deepDive === undefined) {
    throw new Error("the sample course changed");
  }
  await core.call({
    type: "OpenSection",
    courseId: course.id,
    sectionId: deepDive.id,
  });
  window.location.hash = `#/export/${courseId ?? course.id}`;
  render(
    <CoreProvider client={core}>
      <App />
    </CoreProvider>,
  );
}

/** Whether `first` comes before `second` in the document. */
function precedes(first: Node, second: Node) {
  return Boolean(
    first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING,
  );
}

describe("the export route", () => {
  it("lays out the title page, the contents, every opened section in index order with its deep dive under it, and the glossary last", async () => {
    await renderExport();

    const title = await screen.findByRole("heading", {
      level: 1,
      name: "Plate tectonics",
    });
    const nav = await screen.findByRole("navigation", { name: "Contents" });
    const entries = within(nav).getAllByRole("listitem");
    expect(entries).toHaveLength(contents.length);
    contents.forEach((entry, at) =>
      expect(entries[at]).toHaveTextContent(entry),
    );

    const articles = screen.getAllByRole("article");
    expect(articles).toHaveLength(opened.length);
    opened.forEach((name, at) =>
      expect(articles[at]).toHaveAccessibleName(name),
    );
    expect(precedes(title, nav)).toBe(true);
    expect(precedes(nav, articles[0]!)).toBe(true);

    const convergent = screen.getByRole("article", {
      name: "Convergent boundaries",
    });
    expect(
      within(convergent).getByRole("heading", {
        level: 2,
        name: "Convergent boundaries",
      }),
    ).toBeVisible();
    expect(
      within(convergent).getByText(
        "What happens where two plates meet head on, and why one of them almost always loses.",
      ),
    ).toBeVisible();
    expect(
      within(convergent).getByRole("figure", {
        name: /An oceanic plate subducting beneath a continental plate/,
      }),
    ).toBeVisible();
    expect(
      within(convergent).getByRole("figure", { name: new RegExp(DIAGRAM) }),
    ).toBeVisible();
    expect(
      within(convergent).getByRole("button", {
        name: "Source 1: Understanding plate motions, U.S. Geological Survey",
      }),
    ).toBeVisible();
    expect(
      within(convergent).getByRole("heading", { name: "Sources" }),
    ).toBeVisible();
    expect(
      within(convergent).getByRole("link", {
        name: /Subduction zones, R\. J\. Stern, Reviews of Geophysics/,
      }),
    ).toBeVisible();

    const deepDive = screen.getByRole("article", {
      name: "Subduction, step by step",
    });
    expect(
      within(deepDive).getByRole("heading", {
        level: 3,
        name: "Subduction, step by step",
      }),
    ).toBeVisible();
    expect(
      within(deepDive).getByText("From the trench to the volcanic arc."),
    ).toBeVisible();
    expect(precedes(convergent, deepDive)).toBe(true);

    const glossary = screen.getByRole("region", { name: "Glossary" });
    expect(
      within(glossary).getByRole("heading", { level: 2, name: "Glossary" }),
    ).toBeVisible();
    expect(within(glossary).getByText("asthenosphere")).toBeVisible();
    expect(
      within(glossary).getByText(
        /The layer of the mantle directly beneath the plates\./,
      ),
    ).toBeVisible();
    expect(within(glossary).getByText(/Convergent boundaries/)).toBeVisible();
    expect(precedes(articles.at(-1)!, glossary)).toBe(true);
  });

  it("names unopened sections in the contents only, as titles without a link", async () => {
    await renderExport();

    const nav = await screen.findByRole("navigation", { name: "Contents" });
    for (const title of opened) {
      expect(within(nav).getByRole("link", { name: title })).toBeVisible();
    }
    for (const title of unopened) {
      expect(within(nav).getByText(title)).toBeVisible();
      expect(within(nav).queryByRole("link", { name: title })).toBeNull();
      expect(screen.getAllByText(title)).toHaveLength(1);
      expect(screen.queryByRole("article", { name: title })).toBeNull();
    }
  });

  it("shows check questions with their options and leaves out side chats and every control", async () => {
    await renderExport();

    const convergent = await screen.findByRole("article", {
      name: "Convergent boundaries",
    });
    expect(
      within(convergent).getByText(
        "A plate carrying young, warm ocean floor meets a plate carrying old, cold ocean floor. What happens next?",
      ),
    ).toBeVisible();
    for (const option of [
      "The old plate sinks beneath the young one.",
      "The young plate sinks, because it is hotter and softer.",
      "Neither sinks; they build a mountain belt like the Himalaya.",
      "They slide past each other along a fault.",
    ]) {
      expect(within(convergent).getByText(option)).toBeVisible();
    }
    expect(screen.queryByRole("radio")).toBeNull();
    expect(screen.queryByRole("textbox")).toBeNull();
    // The only buttons left are the citation marks in the text.
    for (const button of screen.getAllByRole("button")) {
      expect(button).toHaveAccessibleName(/^Source \d+: /);
    }

    expect(
      screen.queryByText(/Is slab pull the main thing that moves plates/),
    ).toBeNull();
    expect(screen.queryByText(/For plates with a subducting edge/)).toBeNull();
    expect(screen.queryByText(/side chat/i)).toBeNull();
  });

  it("signals ready once, after the diagram is drawn", async () => {
    const signals: {
      state: unknown;
      attribute: string | undefined;
      drawing: boolean;
      drawn: boolean;
    }[] = [];
    const listener = (event: Event) =>
      signals.push({
        state: (event as CustomEvent).detail,
        attribute: document.documentElement.dataset.exportState,
        drawing: screen.queryByText("Drawing the diagram…") !== null,
        drawn: screen.queryByRole("img", { name: DIAGRAM }) !== null,
      });
    window.addEventListener(READY_EVENT, listener);
    try {
      await renderExport();

      await waitFor(() => expect(signals).toHaveLength(1), { timeout: 15_000 });
      expect(signals[0]).toEqual({
        state: "ready",
        attribute: "ready",
        drawing: false,
        drawn: true,
      });
    } finally {
      window.removeEventListener(READY_EVENT, listener);
    }
  });

  it("waits for the fonts before it signals ready", async () => {
    const loading = Promise.withResolvers<void>();
    const fonts = {
      status: "loading",
      ready: loading.promise.then(() => fonts),
    };
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: fonts,
    });

    await renderExport();
    await screen.findByRole("img", { name: DIAGRAM }, { timeout: 15_000 });
    const pause = Promise.withResolvers<void>();
    setTimeout(pause.resolve, 100);
    await pause.promise;
    expect(document.documentElement.dataset.exportState).toBeUndefined();

    fonts.status = "loaded";
    loading.resolve();

    await waitFor(() =>
      expect(document.documentElement.dataset.exportState).toBe("ready"),
    );
  });

  it("says why and signals failed when the course cannot be read", async () => {
    await renderExport("00000000-0000-0000-0000-000000000000");

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The course could not be exported.",
    );
    await waitFor(() =>
      expect(document.documentElement.dataset.exportState).toBe("failed"),
    );
  });
});
