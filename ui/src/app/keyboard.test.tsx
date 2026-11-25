import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { beforeAll, describe, expect, it } from "vitest";

import { App } from "@/App";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import { createValve, drained, startedAt } from "@/test/fake-core";

beforeAll(() => {
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

/**
 * The app on the fake core in `scenario`, every step let through until the
 * test closes the valve. `requests` records what the interface asked of the
 * core, in order.
 */
function renderApp(scenario = "default") {
  const valve = createValve();
  valve.open();
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace: valve.pace,
  });
  const requests: CoreRequest[] = [];
  const client: CoreClient = {
    call(request) {
      requests.push(request);
      return core.call(request);
    },
    subscribe: (listener) => core.subscribe(listener),
  };
  render(
    <CoreProvider client={client}>
      <App />
    </CoreProvider>,
  );
  return { requests, valve, user: userEvent.setup() };
}

/** The title of the open section. */
async function sectionTitle(title: string) {
  const article = await screen.findByRole("article", { name: title });
  return within(article).getByRole("heading", { level: 1, name: title });
}

/** The scrolling column the open section sits in. */
function column(): HTMLElement {
  const article = screen.getByRole("article");
  const scroller = article.parentElement;
  if (scroller === null) throw new Error("the article has no column");
  return scroller;
}

/** Opens Plate tectonics from the library, where it resumes. */
async function openPlateTectonics(user: UserEvent) {
  await user.click(
    await screen.findByRole("button", { name: "Plate tectonics" }),
  );
  await sectionTitle("Convergent boundaries");
  return screen.getByRole("navigation", { name: "Index" });
}

/** Selects `phrase`, which lies in one text node of `root`. */
async function select(root: Element, phrase: string) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    const at = node.nodeValue?.indexOf(phrase) ?? -1;
    if (at === -1) continue;
    const range = document.createRange();
    range.setStart(node, at);
    range.setEnd(node, at + phrase.length);
    await act(async () => {
      const selection = window.getSelection()!;
      selection.removeAllRanges();
      selection.addRange(range);
      await drained();
    });
    return;
  }
  throw new Error(`no text node with “${phrase}”`);
}

describe("focus after each navigation", () => {
  it("signing in moves focus to the library's topic field", async () => {
    const { user } = renderApp("first-launch");
    await screen.findByRole("button", { name: "Continue with ChatGPT" });

    await user.tab();
    await user.keyboard("{Enter}");

    const topic = await screen.findByRole("textbox", { name: "Topic" });
    await waitFor(() => expect(topic).toHaveFocus());
  });

  it("finishing placement opens the first section with focus on its title", async () => {
    const { user } = renderApp("empty-library");
    await user.type(
      await screen.findByRole("textbox", { name: "Topic" }),
      "Plate tectonics{Enter}",
    );
    await screen.findByRole("group", { name: "How well do you know it?" });
    await user.click(screen.getByRole("radio", { name: "New to this" }));
    await user.click(screen.getByRole("radio", { name: "Overview" }));
    await user.click(screen.getByRole("button", { name: "Continue" }));
    for (let number = 1; number <= 4; number += 1) {
      await screen.findByText(`Question ${number} of 4 to 6`);
      await user.keyboard("0{Enter}");
    }

    const title = await sectionTitle("Layers by composition");
    await waitFor(() => expect(title).toHaveFocus());
  });

  it("resuming a course from the library moves focus to the section's title", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);

    const title = await sectionTitle("Convergent boundaries");
    await waitFor(() => expect(title).toHaveFocus());
  });

  it("a section opened from the index takes focus to its title, with the column at the top", async () => {
    const { user } = renderApp();
    const index = await openPlateTectonics(user);
    column().scrollTop = 600;

    await user.click(
      within(index).getByRole("button", { name: "Divergent boundaries" }),
    );

    const title = await sectionTitle("Divergent boundaries");
    await waitFor(() => expect(title).toHaveFocus());
    expect(column().scrollTop).toBe(0);
  });

  it("a deep dive opened from the index takes focus to its title", async () => {
    const { user } = renderApp();
    const index = await openPlateTectonics(user);

    await user.click(
      within(index).getByRole("button", { name: "Subduction, step by step" }),
    );

    const title = await sectionTitle("Subduction, step by step");
    await waitFor(() => expect(title).toHaveFocus());
  });

  it("a section chosen in the titles overlay closes it and takes focus to its title", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);
    const toggle = screen.getByRole("button", { name: "Show the index" });

    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{ArrowUp}{Enter}");

    const title = await sectionTitle("Divergent boundaries");
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await waitFor(() => expect(title).toHaveFocus());
  });

  it("Next keeps focus on Next, and the column starts at the top of the next section", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);
    const next = screen.getByRole("button", { name: "Next" });
    await waitFor(() => expect(next).toBeEnabled());
    column().scrollTop = 600;

    await user.click(next);

    await sectionTitle("Transform boundaries");
    expect(next).toHaveFocus();
    expect(column().scrollTop).toBe(0);
  });

  it("Go deeper keeps focus on Go deeper, and the column starts at the top of the deep dive it opens", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);
    const goDeeper = screen.getByRole("button", { name: "Go deeper" });
    await waitFor(() => expect(goDeeper).not.toHaveAttribute("aria-disabled"));
    act(() => goDeeper.focus());
    column().scrollTop = 600;

    await user.keyboard("{Enter}");

    await sectionTitle("Subduction, step by step");
    expect(goDeeper).toHaveFocus();
    expect(column().scrollTop).toBe(0);
  });

  it("returning to the library moves focus to the topic field", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);

    await user.click(screen.getByRole("button", { name: "Library" }));

    const topic = await screen.findByRole("textbox", { name: "Topic" });
    await waitFor(() => expect(topic).toHaveFocus());
  });
});

describe("the skip control and the tab order", () => {
  it("a skip control at the start of the rail takes focus to the open section's title", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);
    const title = await sectionTitle("Convergent boundaries");
    const rail = screen.getByRole("complementary", { name: "Course" });
    const skip = within(rail).getByRole("link", {
      name: "Skip to the reading column",
    });
    // It is the rail's first stop.
    act(() => rail.querySelector<HTMLElement>("a, button")?.focus());
    expect(skip).toHaveFocus();

    await user.keyboard("{Enter}");

    expect(title).toHaveFocus();
  });

  it("Tab runs through the rail, the header, the column, the foot and the panel, in that order", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);
    await user.click(screen.getByRole("button", { name: /^Glossary/ }));
    screen.getByRole("complementary", { name: "Side panel" });
    const skip = screen.getByRole("link", {
      name: "Skip to the reading column",
    });
    act(() => skip.focus());

    const regions: string[] = [];
    for (let step = 0; step < 200; step += 1) {
      const focused = document.activeElement;
      if (focused === null || focused === document.body) break;
      const region = focused.closest('[aria-label="Course"]')
        ? "rail"
        : focused.closest("main header")
          ? "header"
          : focused.closest("main footer")
            ? "foot"
            : focused.closest("main")
              ? "column"
              : focused.closest('[aria-label="Side panel"]')
                ? "panel"
                : "elsewhere";
      if (regions.at(-1) !== region) regions.push(region);
      await user.tab();
      if (document.activeElement === skip) break;
    }

    expect(regions).toEqual(["rail", "header", "column", "foot", "panel"]);
  });
});

describe("focus returns from each overlay", () => {
  it("Escape closes the course menu only, back on its button, then the side panel, back on its button", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);
    const glossary = screen.getByRole("button", { name: /^Glossary/ });
    await user.click(glossary);
    const menuButton = screen.getByRole("button", { name: "Course menu" });

    await user.click(menuButton);
    expect(await screen.findByRole("menu")).toBeVisible();
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("menu")).toBeNull();
    expect(
      screen.getByRole("complementary", { name: "Side panel" }),
    ).toBeVisible();
    await waitFor(() => expect(menuButton).toHaveFocus());

    await user.keyboard("{Escape}");
    expect(
      screen.queryByRole("complementary", { name: "Side panel" }),
    ).toBeNull();
    expect(glossary).toHaveFocus();
  });

  it("closing the titles overlay with Escape returns focus to its button, and leaves the side panel open", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);
    await user.click(screen.getByRole("button", { name: /^Side chats/ }));
    const toggle = screen.getByRole("button", { name: "Show the index" });

    await user.click(toggle);
    await user.keyboard("{Escape}");

    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveFocus();
    expect(
      screen.getByRole("complementary", { name: "Side panel" }),
    ).toBeVisible();
  });

  it("the other usage windows, closed with Escape, return focus to their button", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);
    const meter = await screen.findByRole("region", { name: "Plan usage" });
    // Signed in, the meter reads usage by itself: there is nothing to connect.
    const more = await within(meter).findByRole("button", {
      name: "1 more window",
    });
    expect(screen.queryByRole("button", { name: "Connect usage" })).toBeNull();

    await user.click(more);
    expect(
      await screen.findByRole("dialog", { name: "Other windows" }),
    ).toBeVisible();
    await user.keyboard("{Escape}");

    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Other windows" }),
      ).toBeNull(),
    );
    await waitFor(() => expect(more).toHaveFocus());
  });
});

describe("single-key shortcuts", () => {
  it("D and A typed in the side chat's prompt field are text, while a passage is selected", async () => {
    const { user, requests } = renderApp();
    await openPlateTectonics(user);
    const article = screen.getByRole("article", {
      name: "Convergent boundaries",
    });
    await user.click(screen.getByRole("button", { name: /^Side chats/ }));
    const panel = screen.getByRole("complementary", { name: "Side panel" });
    await user.click(
      within(panel).getByRole("button", { name: "New side chat" }),
    );
    const field = await within(panel).findByRole("textbox", {
      name: "Ask a question",
    });
    const started = requests.filter((r) => r.type === "StartSideChat").length;

    await select(article, "a process called subduction");
    expect(screen.getByRole("toolbar", { name: "Selection" })).toBeVisible();
    act(() => field.focus());
    await user.keyboard("Dad");

    expect(field).toHaveValue("Dad");
    expect(screen.queryByRole("dialog", { name: /^Definition of/ })).toBeNull();
    expect(requests.some((r) => r.type === "Define")).toBe(false);
    expect(requests.filter((r) => r.type === "StartSideChat")).toHaveLength(
      started,
    );
  });

  it("digits, D and A typed in the topic field are text", async () => {
    const { user, requests } = renderApp();
    const topic = await screen.findByRole("textbox", { name: "Topic" });

    await user.type(topic, "Dna 1 to 4, and 0");
    await act(drained);

    expect(topic).toHaveValue("Dna 1 to 4, and 0");
    expect(topic).toHaveFocus();
    expect(requests.map((r) => r.type)).not.toContain("CreateCourse");
    expect(requests.map((r) => r.type)).not.toContain("OpenCourse");
  });
});
