import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { App } from "@/App";
import { AtlasStoreProvider } from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";
import { startedAt } from "@/test/fake-core";

import { GlossaryList } from "./glossary-list";

const COURSE = "course-1";

/** A definition of `term` made in the section `sectionTitle` at `createdAt`. */
function entry(
  term: string,
  createdAt: string,
  sectionTitle = "Convergent boundaries",
  definition = `What ${term} means.`,
): GlossaryEntry {
  return {
    id: `entry-${term}`,
    courseId: COURSE,
    sectionId: `section-${sectionTitle}`,
    sectionTitle,
    term,
    definition,
    anchor: { versionId: "version-1", block: 0, quote: term },
    wikipedia: null,
    createdAt,
  };
}

/**
 * The glossary list over a store whose core answers `ListGlossary` only when
 * the test says so (`list` or `fail`) and leaves every other request
 * pending. `requests` records what the list asked; `emit` sends an event.
 */
function renderList() {
  const requests: CoreRequest[] = [];
  const listeners = new Set<(event: CoreEvent) => void>();
  let answer = Promise.withResolvers<GlossaryEntry[]>();
  const client: CoreClient = {
    call: ((request: CoreRequest) => {
      requests.push(request);
      if (request.type !== "ListGlossary") return new Promise(() => {});
      return answer.promise.then((entries) => ({
        type: "Glossary",
        courseId: request.courseId,
        entries,
      }));
    }) as CoreClient["call"],
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <GlossaryList courseId={COURSE} />
      </AtlasStoreProvider>
    </CoreProvider>,
  );
  return {
    requests,
    user: userEvent.setup(),
    async list(entries: GlossaryEntry[]) {
      await act(async () => {
        answer.resolve(entries);
        await answer.promise;
      });
    },
    async fail(error: CoreError) {
      const failed = answer;
      answer = Promise.withResolvers<GlossaryEntry[]>();
      await act(async () => {
        failed.reject(error);
        await failed.promise.catch(() => {});
      });
    },
    emit(event: CoreEvent) {
      act(() => {
        for (const listener of listeners) listener(event);
      });
    },
  };
}

/** The terms of the rendered list, top to bottom. */
function terms() {
  return within(screen.getByRole("list", { name: "Glossary" }))
    .getAllByRole("listitem")
    .map((item) => within(item).getByRole("term").textContent);
}

describe("the glossary list", () => {
  it("asks for the course's glossary and says it is loading until it arrives", () => {
    const { requests } = renderList();

    expect(requests).toContainEqual({ type: "ListGlossary", courseId: COURSE });
    expect(screen.getByRole("status")).toHaveTextContent(
      "Loading the glossary…",
    );
    expect(screen.queryByRole("list", { name: "Glossary" })).toBeNull();
  });

  it("says in one sentence that terms defined while reading appear here", async () => {
    const { list } = renderList();

    await list([]);

    expect(
      screen.getByText(
        "No terms yet: the terms you define while reading appear here.",
      ),
    ).toBeVisible();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("list", { name: "Glossary" })).toBeNull();
  });

  it("shows an entry's term, its definition and the section it came from as a control that opens it", async () => {
    const { list, requests, user } = renderList();

    await list([
      entry(
        "asthenosphere",
        "2026-11-17T10:00:00.000Z",
        "Convergent boundaries",
        "The weak, partly molten layer of the mantle beneath the lithosphere.",
      ),
    ]);

    const items = within(
      screen.getByRole("list", { name: "Glossary" }),
    ).getAllByRole("listitem");
    expect(items).toHaveLength(1);
    const [item] = items as [HTMLElement];
    expect(within(item).getByRole("term")).toHaveTextContent("asthenosphere");
    expect(within(item).getByRole("definition")).toHaveTextContent(
      "The weak, partly molten layer of the mantle beneath the lithosphere.",
    );
    const source = within(item).getByRole("button", {
      name: "Convergent boundaries",
    });

    await user.click(source);

    expect(requests).toContainEqual({
      type: "OpenSection",
      courseId: COURSE,
      sectionId: "section-Convergent boundaries",
    });
  });

  it("lists many entries newest first, whatever order the core gives them in", async () => {
    const { list } = renderList();
    const names = Array.from({ length: 24 }, (_, at) => `term ${at + 1}`);

    // The core lists them in the order they were defined, oldest first.
    await list(
      names.map((name, at) =>
        entry(name, new Date(Date.UTC(2026, 10, 1, 10, at)).toISOString()),
      ),
    );

    expect(terms()).toEqual(names.toReversed());
  });

  it("shows a very long definition whole, with its section control after it", async () => {
    const { list } = renderList();
    const definition = `${"A plate boundary where one plate sinks beneath another and returns to the mantle. ".repeat(40)}Supercalifragilisticexpialidociousunbrokenwordwithoutanyspacesatallthatkeepsgoingandgoing.`;

    await list([
      entry(
        "subduction zone",
        "2026-11-17T10:00:00.000Z",
        "Convergent boundaries",
        definition,
      ),
    ]);

    const item = within(
      screen.getByRole("list", { name: "Glossary" }),
    ).getByRole("listitem");
    expect(within(item).getByRole("definition").textContent).toBe(definition);
    expect(
      within(item).getByRole("button", { name: "Convergent boundaries" }),
    ).toBeVisible();
  });

  it("adds a new definition at the top as it arrives, without listing again", async () => {
    const { list, emit, requests } = renderList();
    await list([entry("asthenosphere", "2026-11-17T10:00:00.000Z")]);

    emit({
      type: "GlossaryChanged",
      courseId: COURSE,
      entry: entry("subduction", "2026-11-19T10:00:00.000Z"),
    });

    expect(terms()).toEqual(["subduction", "asthenosphere"]);
    expect(
      requests.filter((request) => request.type === "ListGlossary"),
    ).toHaveLength(1);
  });

  it("says when the glossary could not load, and tries again on request", async () => {
    const { fail, list, requests, user } = renderList();

    await fail({ type: "Store", message: "database is locked" });

    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.getByText("The glossary could not load.")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(
      requests.filter((request) => request.type === "ListGlossary"),
    ).toHaveLength(2);
    await list([entry("asthenosphere", "2026-11-17T10:00:00.000Z")]);
    expect(terms()).toEqual(["asthenosphere"]);
  });
});

describe("the glossary in the reader", () => {
  it("takes a new definition in while the panel is open, and the tab and header button count it", async () => {
    const core = createFakeCore({
      scenario: "default",
      clock: createManualClock(startedAt),
      pace: () => Promise.resolve(),
    });
    render(
      <CoreProvider client={core}>
        <App />
      </CoreProvider>,
    );
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Plate tectonics" }),
    );
    const index = await screen.findByRole("navigation", { name: "Index" });
    await user.click(
      within(index).getByRole("button", { name: "Divergent boundaries" }),
    );
    await screen.findByRole("article", { name: "Divergent boundaries" });

    await user.click(screen.getByRole("button", { name: "Glossary 1" }));
    const panel = screen.getByRole("complementary", { name: "Side panel" });
    const list = await within(panel).findByRole("list", { name: "Glossary" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);

    const { courses } = await core.call({ type: "ListCourses" });
    const courseId = courses.find((c) => c.topic === "Plate tectonics")!.id;
    const { course } = await core.call({ type: "OpenCourse", courseId });
    const divergent = course.chapters
      .flatMap((chapter) => chapter.sections)
      .find((section) => section.title === "Divergent boundaries")!;
    const { section } = await core.call({
      type: "OpenSection",
      courseId,
      sectionId: divergent.id,
    });
    await act(async () => {
      await core.call({
        type: "Define",
        anchor: { versionId: section.version.id, block: 0, quote: "rift" },
        sentence: "The plates pull apart along a rift.",
      });
    });

    await expect
      .poll(() =>
        within(list)
          .getAllByRole("listitem")
          .map((item) => within(item).getByRole("term").textContent),
      )
      .toEqual(["rift", "asthenosphere"]);
    expect(
      within(panel).getByRole("tab", { name: "Glossary 2" }),
    ).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("button", { name: "Glossary 2" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    await user.click(
      within(list).getAllByRole("button", {
        name: "Convergent boundaries",
      })[0]!,
    );
    expect(
      await screen.findByRole("article", { name: "Convergent boundaries" }),
    ).toBeVisible();
  });
});

describe("an entry's Wikipedia article", () => {
  it("is a link beside the section control that opens the found article through the core", async () => {
    const { list, requests, user } = renderList();

    await list([
      {
        ...entry("asthenosphere", "2026-11-17T10:00:00.000Z"),
        wikipedia: {
          type: "Found",
          title: "Asthenosphere",
          extract: "The asthenosphere is the weak region of the upper mantle.",
          url: "https://en.wikipedia.org/wiki/Asthenosphere",
        },
      },
    ]);

    const item = within(
      screen.getByRole("list", { name: "Glossary" }),
    ).getByRole("listitem");
    expect(
      within(item).getByRole("button", { name: "Convergent boundaries" }),
    ).toBeVisible();
    const link = within(item).getByRole("link", {
      name: "Wikipedia article on asthenosphere",
    });
    expect(link).toHaveTextContent("Wikipedia");
    expect(link).toHaveAttribute(
      "href",
      "https://en.wikipedia.org/wiki/Asthenosphere",
    );

    await user.click(link);

    expect(requests).toContainEqual({
      type: "OpenExternal",
      url: "https://en.wikipedia.org/wiki/Asthenosphere",
    });
    expect(
      requests.filter((request) => request.type === "LookupWikipedia"),
    ).toEqual([]);
  });

  it("is absent from entries never looked up, with no article or whose lookup failed, and the list looks none up", async () => {
    const { list, requests } = renderList();

    await list([
      entry("rift", "2026-11-17T10:00:00.000Z"),
      {
        ...entry("fingernails", "2026-11-17T11:00:00.000Z"),
        wikipedia: { type: "NoArticle" },
      },
      {
        ...entry("trench", "2026-11-17T12:00:00.000Z"),
        wikipedia: { type: "Failed", error: { type: "Offline" } },
      },
    ]);

    const items = within(
      screen.getByRole("list", { name: "Glossary" }),
    ).getAllByRole("listitem");
    expect(items).toHaveLength(3);
    for (const item of items) {
      expect(
        within(item).getByRole("button", { name: "Convergent boundaries" }),
      ).toBeVisible();
      expect(within(item).queryByRole("link")).toBeNull();
      expect(item).not.toHaveTextContent("Wikipedia");
    }
    expect(
      requests.filter((request) => request.type === "LookupWikipedia"),
    ).toEqual([]);
  });
});
