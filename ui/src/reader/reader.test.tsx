import { render, screen, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { App } from "@/App";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import { createValve, startedAt } from "@/test/fake-core";

/**
 * The app on the fake core in `scenario`, every step let through at once
 * unless `pace` holds it. `requests` records what the interface asked of
 * the core, in order.
 */
function renderApp(scenario = "default", pace = () => Promise.resolve()) {
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace,
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
  return { requests, user: userEvent.setup() };
}

/** Opens Plate tectonics from the library; returns its index. */
async function openPlateTectonics(user: UserEvent) {
  await user.click(
    await screen.findByRole("button", { name: "Plate tectonics" }),
  );
  return screen.findByRole("navigation", { name: "Index" });
}

/** Opens the section titled `title` from the index; returns its article. */
async function openSection(user: UserEvent, title: string) {
  const index = screen.getByRole("navigation", { name: "Index" });
  await user.click(within(index).getByRole("button", { name: title }));
  return screen.findByRole("article", { name: title });
}

/** Starts a course on `topic` and answers placement through to its index. */
async function placeNewCourse(user: UserEvent, topic: string) {
  await user.type(await screen.findByRole("textbox", { name: "Topic" }), topic);
  await user.click(screen.getByRole("button", { name: "Start course" }));
  await user.click(await screen.findByRole("radio", { name: "New to this" }));
  await user.click(screen.getByRole("radio", { name: "Overview" }));
  await user.click(screen.getByRole("button", { name: "Continue" }));
  for (let number = 1; number <= 4; number += 1) {
    await screen.findByText(`Question ${number} of 4 to 6`);
    await user.click(screen.getByRole("radio", { name: "Not sure" }));
    await user.click(screen.getByRole("button", { name: "Continue" }));
  }
  return screen.findByRole("navigation", { name: "Index" });
}

describe("the reader frame", () => {
  it("shows the course title and read count above the index, with Library reachable", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);

    expect(
      screen.getByRole("heading", { name: "Plate tectonics" }),
    ).toBeVisible();
    // Seven broad-pass sections and the three deep dives of Convergent
    // boundaries; four of them read in the sample course.
    expect(screen.getByText("4 of 10 sections read")).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Library" }));
    expect(await screen.findByRole("textbox", { name: "Topic" })).toBeVisible();
  });

  it("lists chapters and sections with their read state as text, and selecting one opens it", async () => {
    const { user } = renderApp();
    const index = await openPlateTectonics(user);

    expect(
      within(index).getByText("Plates and their boundaries"),
    ).toBeVisible();
    expect(
      within(index).getByRole("button", { name: "Layers by composition" }),
    ).toHaveAccessibleDescription("Read");
    expect(
      within(index).getByRole("button", { name: "Transform boundaries" }),
    ).toHaveAccessibleDescription("Unread");
    expect(
      within(index).getByRole("button", { name: "Why the denser plate sinks" }),
    ).toBeVisible();

    const article = await openSection(user, "Divergent boundaries");
    expect(article).toBeVisible();
    expect(
      within(index).getByRole("button", { name: "Divergent boundaries" }),
    ).toHaveAttribute("aria-current", "page");
  });

  it("shows the section with its title, summary, prose, a text line for a figure, sources and the explain actions", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);

    const article = await openSection(user, "Convergent boundaries");

    expect(
      within(article).getByRole("heading", {
        level: 1,
        name: "Convergent boundaries",
      }),
    ).toBeVisible();
    expect(article).toHaveTextContent(
      "What happens where two plates meet head on, and why one of them almost always loses.",
    );
    expect(article).toHaveTextContent(
      "Plates move toward each other at a few centimetres a year",
    );
    expect(
      within(article).getByText(
        /An oceanic plate subducting beneath a continental plate/,
      ),
    ).toBeVisible();
    const sources = within(article).getByRole("list", { name: "Sources" });
    expect(within(sources).getAllByRole("listitem")).toHaveLength(2);
    expect(sources).toHaveTextContent("Understanding plate motions");
    const explain = within(article).getByRole("group", {
      name: "Explain this section differently",
    });
    expect(
      within(explain)
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual(["Simpler", "More technical", "More examples"]);
  });

  it("shows the breadcrumb, the narrow-window index button and the course menu with Export PDF", async () => {
    const { requests, user } = renderApp();
    await openPlateTectonics(user);
    await openSection(user, "Convergent boundaries");

    const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(crumbs).toHaveTextContent("Plates and their boundaries");
    expect(crumbs).toHaveTextContent("Convergent boundaries");

    const indexButton = screen.getByRole("button", { name: "Show the index" });
    expect(indexButton).toHaveAttribute("aria-expanded", "false");
    await user.click(indexButton);
    expect(indexButton).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{Escape}");
    expect(indexButton).toHaveAttribute("aria-expanded", "false");
    expect(indexButton).toHaveFocus();

    await user.click(screen.getByRole("button", { name: "Course menu" }));
    await user.click(
      await screen.findByRole("menuitem", { name: "Export PDF" }),
    );
    expect(requests).toContainEqual({
      type: "ExportPdf",
      courseId: expect.any(String),
    });
  });

  it("shows no version switch with one version, and a switch once there are more", async () => {
    const { requests, user } = renderApp();
    await openPlateTectonics(user);
    const article = await openSection(user, "Convergent boundaries");
    expect(screen.queryByRole("button", { name: /^Version/ })).toBeNull();

    await user.click(
      within(article).getByRole("button", { name: "More examples" }),
    );

    const versions = await screen.findByRole("button", {
      name: "Version: More examples, 2 of 2",
    });
    await user.click(versions);
    await user.click(
      await screen.findByRole("menuitemradio", { name: /Original/ }),
    );
    expect(requests).toContainEqual(
      expect.objectContaining({ type: "SelectVersion" }),
    );
    expect(
      await screen.findByRole("button", { name: "Version: Original, 1 of 2" }),
    ).toBeVisible();
  });

  it("shows Stop while a section streams and Retry once it stopped", async () => {
    const valve = createValve();
    const { requests, user } = renderApp("default", valve.pace);
    await openPlateTectonics(user);

    const article = await openSection(user, "Transform boundaries");
    await user.click(
      await within(article).findByRole("button", { name: "Stop" }),
    );
    expect(requests).toContainEqual(
      expect.objectContaining({ type: "StopGeneration" }),
    );
    valve.step();

    expect(
      await within(article).findByText("This section is incomplete."),
    ).toBeVisible();
    expect(within(article).queryByRole("button", { name: "Stop" })).toBeNull();
    valve.open();
    await user.click(within(article).getByRole("button", { name: "Retry" }));
    expect(requests).toContainEqual(
      expect.objectContaining({ type: "RetrySection" }),
    );
    await expect
      .poll(() => article.textContent)
      .toContain("we feel as earthquakes.");
    expect(within(article).queryByRole("button", { name: "Retry" })).toBeNull();
  });

  it("offers Next and Go deeper at the foot; Go deeper adds deep dives to the index", async () => {
    const { user } = renderApp();
    const index = await openPlateTectonics(user);
    await openSection(user, "Divergent boundaries");

    expect(screen.getByRole("button", { name: "Next" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "Go deeper" }));

    expect(
      await within(index).findByRole("button", {
        name: "Divergent boundaries: in the field",
      }),
    ).toBeVisible();
  });

  it("shows the plan's usage at the foot of the index, with no connection step", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);

    const meter = await screen.findByRole("region", { name: "Plan usage" });
    expect(
      await within(meter).findByRole("img", {
        name: /^5-hour window, 38% used/,
      }),
    ).toBeInTheDocument();
    expect(
      within(meter).getByRole("img", { name: /^Weekly window, 61% used/ }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Connect usage" })).toBeNull();
  });
});

describe("the side panel", () => {
  it("opens from the header buttons on the right tab, and closing returns focus to the button", async () => {
    const { user } = renderApp();
    await openPlateTectonics(user);
    await openSection(user, "Convergent boundaries");
    expect(
      screen.queryByRole("complementary", { name: "Side panel" }),
    ).toBeNull();

    const glossary = screen.getByRole("button", { name: "Glossary 1" });
    await user.click(glossary);
    const panel = screen.getByRole("complementary", { name: "Side panel" });
    expect(
      within(panel).getByRole("tab", { name: "Glossary 1" }),
    ).toHaveAttribute("aria-selected", "true");
    expect(glossary).toHaveAttribute("aria-pressed", "true");
    await user.click(
      within(panel).getByRole("button", { name: "Close the side panel" }),
    );
    expect(
      screen.queryByRole("complementary", { name: "Side panel" }),
    ).toBeNull();
    expect(glossary).toHaveFocus();

    const chats = screen.getByRole("button", { name: "Side chats 1" });
    await user.click(chats);
    expect(
      within(
        screen.getByRole("complementary", { name: "Side panel" }),
      ).getByRole("tab", { name: "Side chats 1" }),
    ).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Escape}");
    expect(
      screen.queryByRole("complementary", { name: "Side panel" }),
    ).toBeNull();
    expect(chats).toHaveFocus();
  });

  it("lists the course's side chats and glossary, shows the model in use, and starts a new chat", async () => {
    const { requests, user } = renderApp();
    await openPlateTectonics(user);

    await user.click(screen.getByRole("button", { name: "Side chats 1" }));
    const panel = screen.getByRole("complementary", { name: "Side panel" });
    const chats = await within(panel).findByRole("list", {
      name: "Side chats",
    });
    expect(within(chats).getAllByRole("listitem")).toHaveLength(1);
    expect(chats).toHaveTextContent(
      "Its weight drags the rest of the plate behind it",
    );
    expect(await within(panel).findByText("GPT-5.5")).toBeVisible();

    await user.click(
      within(panel).getByRole("button", { name: "New side chat" }),
    );
    expect(requests).toContainEqual({
      type: "StartSideChat",
      courseId: expect.any(String),
      anchor: null,
    });
    await expect
      .poll(() => within(chats).getAllByRole("listitem").length)
      .toBe(2);

    await user.click(within(panel).getByRole("tab", { name: "Glossary 1" }));
    const terms = await within(panel).findByRole("list", { name: "Glossary" });
    expect(terms).toHaveTextContent("asthenosphere");
    expect(terms).toHaveTextContent("Convergent boundaries");
  });

  it("says when a course has no side chats and no glossary yet", async () => {
    const { user } = renderApp("empty-library");
    await placeNewCourse(user, "Plate tectonics");

    await user.click(screen.getByRole("button", { name: "Side chats 0" }));
    const panel = screen.getByRole("complementary", { name: "Side panel" });
    expect(await within(panel).findByText(/No side chats yet/)).toBeVisible();

    await user.click(within(panel).getByRole("tab", { name: "Glossary 0" }));
    expect(await within(panel).findByText(/No terms yet/)).toBeVisible();
  });
});
