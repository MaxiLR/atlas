import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect } from "react";
import { beforeAll, describe, expect, it } from "vitest";

import {
  AtlasStoreProvider,
  useAtlasStore,
  type AtlasStore,
} from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import { SectionView } from "@/reader/section/section-view";
import { createValve, drained, findSection, startedAt } from "@/test/fake-core";

beforeAll(() => {
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

const SUBDUCTION_SENTENCE =
  "One plate bends and slides beneath the other, a process called subduction, and sinks into the asthenosphere below.";
const SUBDUCTION_DEFINITION =
  "The sinking of one plate beneath another where two plates converge. The denser plate bends at a trench and descends into the mantle, carrying water that feeds volcanoes above it.";
const SUBDUCTION_ARTICLE =
  "Subduction is a geological process in which the oceanic lithosphere and some continental lithosphere is recycled into the Earth's mantle at convergent boundaries between tectonic plates.";
const SLAB_PULL =
  "Its weight drags the rest of the plate behind it, a force called slab pull";

function Store({ onStore }: { onStore: (store: AtlasStore) => void }) {
  const store = useAtlasStore();
  useEffect(() => {
    onStore(store);
  }, [store, onStore]);
  return null;
}

/**
 * The reading column of Convergent boundaries (its original version stored,
 * with the sample course's defined term and anchored side chat) over the
 * fake core in `scenario`. Every step of a generation is let through until
 * the test closes the valve; `requests` records what the column asked of
 * the core once it was shown.
 */
async function renderSection(scenario = "default") {
  const valve = createValve();
  valve.open();
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace: valve.pace,
  });
  const { courses } = await core.call({ type: "ListCourses" });
  const courseId = courses.find((c) => c.topic === "Plate tectonics")!.id;
  const { course } = await core.call({ type: "OpenCourse", courseId });
  const sectionId = findSection(course, "Convergent boundaries").id;
  const opened = await core.call({ type: "OpenSection", courseId, sectionId });
  const originalId = opened.section.version.id;

  const requests: CoreRequest[] = [];
  const client: CoreClient = {
    call(request) {
      requests.push(request);
      return core.call(request);
    },
    subscribe: (listener) => core.subscribe(listener),
  };
  let store: AtlasStore | null = null;
  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <Store onStore={(given) => (store = given)} />
        <SectionView course={course} sectionId={sectionId} />
      </AtlasStoreProvider>
    </CoreProvider>,
  );
  await act(async () => {
    await store!.send({ type: "OpenSection", courseId, sectionId });
    await drained();
  });
  const article = screen.getByRole("article", {
    name: "Convergent boundaries",
  });
  return {
    core,
    valve,
    requests,
    store: store!,
    user: userEvent.setup(),
    article,
    ids: { courseId, sectionId },
    originalId,
  };
}

/** The text node of `root` that holds `phrase`. */
function textNodeWith(root: Element, phrase: string): Text {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    if (node.nodeValue?.includes(phrase)) return node as Text;
  }
  throw new Error(`no text node with “${phrase}”`);
}

async function selectRange(range: Range) {
  await act(async () => {
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
    await drained();
  });
}

/** Selects `phrase`, which lies in one text node of `root`. */
async function select(root: Element, phrase: string) {
  const node = textNodeWith(root, phrase);
  const at = node.nodeValue!.indexOf(phrase);
  const range = document.createRange();
  range.setStart(node, at);
  range.setEnd(node, at + phrase.length);
  await selectRange(range);
}

async function clearSelection() {
  await act(async () => {
    window.getSelection()!.removeAllRanges();
    await drained();
  });
}

const menu = () => screen.queryByRole("toolbar", { name: "Selection" });

describe("the selection menu", () => {
  it("offers Define and Ask at a selection inside one prose block", async () => {
    const { article } = await renderSection();

    await select(article, "a process called subduction");

    const toolbar = menu();
    expect(toolbar).not.toBeNull();
    expect(within(toolbar!).getAllByRole("button")).toHaveLength(2);
    expect(
      within(toolbar!).getByRole("button", { name: "Define" }),
    ).toHaveAttribute("aria-keyshortcuts", "D");
    expect(
      within(toolbar!).getByRole("button", { name: "Ask" }),
    ).toHaveAttribute("aria-keyshortcuts", "A");
    expect(
      within(toolbar!).getByRole("button", { name: "Define" }),
    ).toBeVisible();
    expect(within(toolbar!).getByRole("button", { name: "Ask" })).toBeVisible();
  });

  it("shows nothing for a selection across blocks or outside prose", async () => {
    const { article } = await renderSection();

    const across = document.createRange();
    const first = textNodeWith(article, "a process called subduction");
    const second = textNodeWith(article, "Which plate goes under");
    across.setStart(first, first.nodeValue!.indexOf("a process"));
    across.setEnd(second, "Which plate".length);
    await selectRange(across);
    expect(menu()).toBeNull();

    const title = screen.getByRole("heading", {
      level: 1,
      name: "Convergent boundaries",
    });
    await select(title, "Convergent");
    expect(menu()).toBeNull();
  });

  it("goes when the selection is cleared, and on Escape", async () => {
    const { article, user } = await renderSection();

    await select(article, "a process called subduction");
    expect(menu()).not.toBeNull();
    await clearSelection();
    expect(menu()).toBeNull();

    await select(article, "a process called subduction");
    expect(menu()).not.toBeNull();
    await user.keyboard("{Escape}");
    expect(menu()).toBeNull();
  });

  it("D defines the selection: the request carries the version, the block, the quoted text and its sentence", async () => {
    const { article, user, requests, originalId } = await renderSection();

    await select(article, "subduction");
    await user.keyboard("d");

    expect(requests).toContainEqual({
      type: "Define",
      anchor: { versionId: originalId, block: 0, quote: "subduction" },
      sentence: SUBDUCTION_SENTENCE,
    });
    expect(menu()).toBeNull();
    expect(
      await screen.findByRole("dialog", { name: "Definition of subduction" }),
    ).toBeVisible();
  });

  it("Define finds the sentence around the selection, leaving citation markers out", async () => {
    const { article, user, requests, originalId } = await renderSection();

    await select(article, "fingernails");
    await user.click(within(menu()!).getByRole("button", { name: "Define" }));

    expect(requests).toContainEqual({
      type: "Define",
      anchor: { versionId: originalId, block: 0, quote: "fingernails" },
      sentence:
        "Plates move toward each other at a few centimetres a year, about as fast as fingernails grow.",
    });
  });

  it("A starts a side chat anchored to the passage and opens the side panel on it", async () => {
    const { article, user, requests, store, ids, originalId } =
      await renderSection();
    const passage =
      "Oceanic lithosphere is thinner but denser than continental lithosphere";

    await select(article, passage);
    await user.keyboard("a");

    expect(requests).toContainEqual({
      type: "StartSideChat",
      courseId: ids.courseId,
      anchor: { versionId: originalId, block: 1, quote: passage },
    });
    await waitFor(() =>
      expect(store.state.getState().navigation.panel.open).toBe(true),
    );
    const { panel } = store.state.getState().navigation;
    expect(panel.tab).toBe("sideChats");
    const started = store.state
      .getState()
      .sideChats.byCourse[ids.courseId]!.find(
        (chat) => chat.anchor?.quote === passage,
      );
    expect(started).toBeDefined();
    expect(panel.chatId).toBe(started!.id);
    expect(menu()).toBeNull();
    const mark = await within(article).findByRole("button", {
      name: /side chat on “Oceanic lithosphere/i,
    });
    expect(mark).toHaveAttribute("aria-expanded", "true");
  });

  it("D and A do nothing without a selection", async () => {
    const { user, requests } = await renderSection();
    const before = requests.length;

    await user.keyboard("da");
    await drained();

    expect(requests.slice(before)).toEqual([]);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("the definition popover", () => {
  it("shows loading, then the definition as it streams, then the whole definition with its Wikipedia summary", async () => {
    const { article, user, valve, core } = await renderSection();

    valve.close();
    await select(article, "subduction");
    await user.click(within(menu()!).getByRole("button", { name: "Define" }));

    const popover = await screen.findByRole("dialog", {
      name: "Definition of subduction",
    });
    expect(within(popover).getByText("subduction")).toBeVisible();
    expect(within(popover).getByRole("status")).toHaveTextContent(
      "Writing the definition",
    );
    expect(popover).toHaveAttribute("aria-busy", "true");

    await act(async () => {
      valve.step();
      await drained();
    });
    expect(popover).toHaveTextContent(
      "The sinking of one plate beneath another where two plates converge.",
    );
    expect(popover).not.toHaveTextContent("carrying water");
    expect(popover).toHaveAttribute("aria-busy", "true");

    await act(async () => {
      valve.open();
      await drained();
    });
    await waitFor(() =>
      expect(popover).toHaveTextContent(SUBDUCTION_DEFINITION),
    );
    expect(popover).toHaveAttribute("aria-busy", "false");
    expect(
      within(popover).queryByRole("status", { name: /Writing/ }),
    ).toBeNull();
    await waitFor(() => expect(popover).toHaveTextContent(SUBDUCTION_ARTICLE));

    await user.click(
      within(popover).getByRole("link", { name: "Open article" }),
    );
    expect(core.opened).toContain("https://en.wikipedia.org/wiki/Subduction");
  });

  it("says the definition could not be written when the generation fails", async () => {
    const { article, user } = await renderSection("failed-after-retries");

    await select(article, "subduction");
    await user.keyboard("d");

    const popover = await screen.findByRole("dialog", {
      name: "Definition of subduction",
    });
    await waitFor(() =>
      expect(popover).toHaveTextContent("The definition could not be written."),
    );
    expect(popover).toHaveTextContent("The request failed after 4 attempts.");
    expect(popover).toHaveAttribute("aria-busy", "false");
    expect(
      within(popover).getByRole("button", { name: "Try again" }),
    ).toBeEnabled();
  });

  it("says when no Wikipedia article matches the term", async () => {
    const { article, user } = await renderSection();

    await select(article, "fingernails");
    await user.keyboard("d");

    const popover = await screen.findByRole("dialog", {
      name: "Definition of fingernails",
    });
    await waitFor(() =>
      expect(popover).toHaveTextContent("No Wikipedia article matches"),
    );
    expect(
      within(popover).queryByRole("link", { name: "Open article" }),
    ).toBeNull();
  });

  it("says when Wikipedia could not be reached", async () => {
    const { article, user, requests } = await renderSection("offline");

    const term = await within(article).findByRole("button", {
      name: "asthenosphere",
    });
    await user.click(term);

    const popover = await screen.findByRole("dialog", {
      name: "Definition of asthenosphere",
    });
    expect(popover).toHaveTextContent(
      "The layer of the mantle directly beneath the plates.",
    );
    await waitFor(() =>
      expect(popover).toHaveTextContent("Wikipedia could not be reached."),
    );
    expect(requests.some((request) => request.type === "Define")).toBe(false);
  });

  it("leaves a dotted term that reopens the popover from the store by keyboard, without a request", async () => {
    const { article, user, requests } = await renderSection();

    await select(article, "subduction");
    await user.keyboard("d");
    const popover = await screen.findByRole("dialog", {
      name: "Definition of subduction",
    });
    await waitFor(() => expect(popover).toHaveTextContent(SUBDUCTION_ARTICLE));

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    const term = within(article).getByRole("button", { name: "subduction" });
    expect(term).toHaveFocus();

    const before = requests.length;
    await user.keyboard("{Enter}");

    const reopened = await screen.findByRole("dialog", {
      name: "Definition of subduction",
    });
    expect(reopened).toHaveTextContent(SUBDUCTION_DEFINITION);
    expect(reopened).toHaveTextContent(SUBDUCTION_ARTICLE);
    await drained();
    expect(requests.slice(before)).toEqual([]);
  });
});

describe("marks in the text", () => {
  it("show on the version they belong to and not on another", async () => {
    const { article, store, ids, originalId } = await renderSection();

    expect(
      await within(article).findByRole("button", { name: "asthenosphere" }),
    ).toBeVisible();
    expect(
      await within(article).findByRole("button", {
        name: /side chat on “Its weight drags/i,
      }),
    ).toBeVisible();

    await act(async () => {
      await store.send({
        type: "ExplainDifferently",
        ...ids,
        style: "Simpler",
      });
      await drained();
    });
    expect(article).toHaveTextContent("asthenosphere");
    expect(article).toHaveTextContent(SLAB_PULL);
    expect(
      within(article).queryByRole("button", { name: "asthenosphere" }),
    ).toBeNull();
    expect(
      within(article).queryByRole("button", { name: /side chat on/i }),
    ).toBeNull();

    await act(async () => {
      await store.send({
        type: "SelectVersion",
        ...ids,
        versionId: originalId,
      });
      await drained();
    });
    expect(
      within(article).getByRole("button", { name: "asthenosphere" }),
    ).toBeVisible();
    expect(
      within(article).getByRole("button", {
        name: /side chat on “Its weight drags/i,
      }),
    ).toBeVisible();
  });

  it("a side chat's margin mark reopens that chat in the side panel", async () => {
    const { article, user, requests, store, ids } = await renderSection();

    const mark = await within(article).findByRole("button", {
      name: /side chat on “Its weight drags/i,
    });
    expect(mark).toHaveAttribute("aria-expanded", "false");
    const chat = store.state
      .getState()
      .sideChats.byCourse[ids.courseId]!.find(
        (each) => each.anchor?.quote === SLAB_PULL,
      )!;

    await user.click(mark);

    expect(requests).toContainEqual({ type: "OpenSideChat", chatId: chat.id });
    await waitFor(() =>
      expect(store.state.getState().navigation.panel).toEqual({
        open: true,
        tab: "sideChats",
        chatId: chat.id,
      }),
    );
    expect(mark).toHaveAttribute("aria-expanded", "true");
  });
});
