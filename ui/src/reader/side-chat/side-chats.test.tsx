import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { useEffect } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  AtlasStoreProvider,
  useAtlas,
  useAtlasStore,
  type AtlasStore,
} from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock, type FakeWorld } from "@/core/fake";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { SideChat } from "@/core/generated/SideChat";
import { SidePanel } from "@/reader/side-panel/side-panel";
import { selectOpenCourse } from "@/store";
import { createValve, startedAt } from "@/test/fake-core";

import { SideChats } from "./side-chats";

afterEach(() => {
  vi.useRealTimers();
});

const PASSAGE =
  "Its weight drags the rest of the plate behind it, a force called slab pull";
const QUESTION =
  "Is slab pull the main thing that moves plates, or is it convection in the mantle?";
const SOURCE_1 =
  "Source 1: On the relative importance of the driving forces of plate motion, Forsyth and Uyeda";
const FOLLOW_UPS = [
  "What is ridge push, and how much does it add?",
  "How do we know which plates move fastest?",
  "What happens to a slab once it has sunk?",
];
/** The first sentence of the fake core's reply, the first piece to stream. */
const FIRST_CHUNK =
  "For plates with a subducting edge, slab pull is the larger force.";
const LAST_CHUNK =
  "the sinking slab is the cold, downward limb of the mantle's circulation.";

function Store({ onStore }: { onStore: (store: AtlasStore) => void }) {
  const store = useAtlasStore();
  useEffect(() => {
    onStore(store);
  }, [store, onStore]);
  return null;
}

/** The side panel on the Side chats tab, as the reader shows it. */
function Panel() {
  const course = useAtlas(selectOpenCourse);
  if (course === null) return null;
  return (
    <SidePanel
      course={course}
      tab="sideChats"
      onClose={() => {}}
      sideChats={<SideChats courseId={course.id} />}
      glossary={null}
    />
  );
}

function plateTectonics(world: FakeWorld) {
  const course = world.courses.find(({ topic }) => topic === "Plate tectonics");
  if (course === undefined) throw new Error("no Plate tectonics");
  return course;
}

/** The sample course's one side chat, anchored to a passage. */
function sampleChat(world: FakeWorld): SideChat {
  return plateTectonics(world).chats[0]!;
}

/**
 * The side panel over the fake core in `scenario`, with Plate tectonics
 * open. `setup` changes the world before anything is asked; `pace` holds
 * the core's steps. `requests` records what the panel asked of the core.
 */
async function renderPanel({
  scenario = "default",
  pace = () => Promise.resolve(),
  setup,
}: {
  scenario?: string;
  pace?: () => Promise<void>;
  setup?: (world: FakeWorld) => void;
} = {}) {
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace,
  });
  setup?.(core.world);
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
        <Panel />
      </AtlasStoreProvider>
    </CoreProvider>,
  );
  const course = plateTectonics(core.world);
  await act(async () => {
    await store!.send({ type: "OpenCourse", courseId: course.id });
  });
  const panel = await screen.findByRole("complementary", {
    name: "Side panel",
  });
  return {
    core,
    requests,
    panel,
    courseId: course.id,
    user: userEvent.setup(),
  };
}

/** Opens the chat whose list entry is named by `name`; returns the thread. */
async function openChat(user: UserEvent, panel: HTMLElement, name: RegExp) {
  const list = await within(panel).findByRole("list", { name: "Side chats" });
  await user.click(within(list).getByRole("button", { name }));
  return within(panel).findByRole("region", { name: "Side chat" });
}

/** Starts a chat from the panel's New side chat; returns the thread. */
async function startChat(user: UserEvent, panel: HTMLElement) {
  await within(panel).findByRole("list", { name: "Side chats" });
  await user.click(
    within(panel).getByRole("button", { name: "New side chat" }),
  );
  return within(panel).findByRole("region", { name: "Side chat" });
}

describe("the side-chat list", () => {
  it("says in one sentence that there are none yet", async () => {
    const { panel } = await renderPanel({
      setup: (world) => {
        plateTectonics(world).chats.length = 0;
      },
    });

    const empty = await within(panel).findByText(
      "No side chats yet: ask about a selected passage, or start one with New side chat.",
    );
    expect(empty).toBeVisible();
    const tab = within(panel).getByRole("tabpanel");
    expect(tab.textContent).not.toMatch(/Start one/);
    expect(within(panel).queryByRole("list", { name: "Side chats" })).toBe(
      null,
    );
  });

  it("lists several chats newest first, each by its passage or first question, and opens one", async () => {
    const { panel, requests, user, core } = await renderPanel({
      setup: (world) => {
        const course = plateTectonics(world);
        course.chats.unshift({
          id: "chat-unanchored",
          courseId: course.id,
          anchor: null,
          sectionTitle: null,
          messages: [
            {
              id: "message-1",
              role: "Candidate",
              text: "Why do continents not sink?",
              state: "Complete",
              citations: [],
              sources: [],
              createdAt: startedAt,
            },
          ],
          followUps: [],
          createdAt: startedAt,
          updatedAt: startedAt,
        });
      },
    });

    const list = await within(panel).findByRole("list", { name: "Side chats" });
    const entries = within(list).getAllByRole("listitem");
    expect(entries).toHaveLength(2);
    expect(entries[0]).toHaveTextContent("Why do continents not sink?");
    expect(entries[1]).toHaveTextContent(PASSAGE);
    expect(entries[1]).toHaveTextContent("Convergent boundaries");

    await user.click(
      within(entries[1]!).getByRole("button", { name: /slab pull/ }),
    );
    const anchored = plateTectonics(core.world).chats[1]!;
    expect(anchored.anchor).not.toBeNull();
    expect(requests).toContainEqual({
      type: "OpenSideChat",
      chatId: anchored.id,
    });
    expect(requests).not.toContainEqual({
      type: "OpenSideChat",
      chatId: "chat-unanchored",
    });
    const thread = await within(panel).findByRole("region", {
      name: "Side chat",
    });
    expect(thread).toHaveTextContent(QUESTION);
  });
});

describe("a side chat's thread", () => {
  it("quotes its passage with its section, then the question, the search row, the reply with its citations and sources, and three follow-ups", async () => {
    const { panel, user } = await renderPanel();
    const thread = await openChat(user, panel, /slab pull/);

    const quote = within(thread).getByRole("blockquote");
    expect(quote).toHaveTextContent(PASSAGE);
    expect(quote).toHaveTextContent("Convergent boundaries");
    expect(
      quote.compareDocumentPosition(within(thread).getByText(QUESTION)) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    expect(within(thread).getByText("Searched the web")).toBeVisible();
    expect(within(thread).getByText("2 sources")).toBeVisible();
    expect(thread).toHaveTextContent(FIRST_CHUNK);
    expect(
      within(thread).getByRole("button", { name: SOURCE_1 }),
    ).toBeVisible();
    const sources = within(thread).getByRole("list", { name: "Sources" });
    expect(within(sources).getAllByRole("listitem")).toHaveLength(2);

    for (const prompt of FOLLOW_UPS) {
      expect(
        within(thread).getByRole("button", { name: prompt }),
      ).toBeVisible();
    }
    expect(
      within(panel).getByRole("textbox", { name: "Ask a follow-up" }),
    ).toBeVisible();
    expect(
      within(panel).getByRole("button", { name: "All side chats" }),
    ).toBeVisible();
  });

  it("a chat started from the panel has no passage and focuses the prompt", async () => {
    const { panel, requests, user, courseId } = await renderPanel();
    const thread = await startChat(user, panel);

    expect(requests).toContainEqual({
      type: "StartSideChat",
      courseId,
      anchor: null,
    });
    expect(within(thread).queryByRole("blockquote")).toBeNull();
    const field = within(panel).getByRole("textbox", {
      name: "Ask a question",
    });
    expect(field).toHaveFocus();
    expect(
      within(thread).queryByRole("button", { name: FOLLOW_UPS[0] }),
    ).toBeNull();
  });

  it("follow-ups are at most three, and choosing one sends it", async () => {
    const { panel, requests, user, core } = await renderPanel({
      setup: (world) => {
        sampleChat(world).followUps = [...FOLLOW_UPS, "A fourth prompt?"];
      },
    });
    const thread = await openChat(user, panel, /slab pull/);

    expect(
      within(thread).queryByRole("button", { name: "A fourth prompt?" }),
    ).toBeNull();
    await user.click(
      within(thread).getByRole("button", { name: FOLLOW_UPS[1] }),
    );

    expect(requests).toContainEqual({
      type: "SendSideChatMessage",
      chatId: sampleChat(core.world).id,
      text: FOLLOW_UPS[1],
    });
  });

  it("a long reply is shown whole, with the prompt still in reach", async () => {
    const paragraphs = Array.from(
      { length: 40 },
      (_, at) => `Paragraph ${at + 1} of a long reply about slab pull.`,
    );
    const { panel, user } = await renderPanel({
      setup: (world) => {
        const reply = sampleChat(world).messages[1]!;
        reply.text = paragraphs.join("\n\n");
        reply.citations = [];
        reply.sources = [];
      },
    });
    const thread = await openChat(user, panel, /slab pull/);

    expect(within(thread).getByText(paragraphs[0]!)).toBeVisible();
    expect(within(thread).getByText(paragraphs[39]!)).toBeVisible();
    expect(
      within(panel).getByRole("textbox", { name: "Ask a follow-up" }),
    ).toBeVisible();
  });

  it("hostile Markdown in a reply produces no HTML element of its own", async () => {
    const { panel, user } = await renderPanel({
      setup: (world) => {
        const reply = sampleChat(world).messages[1]!;
        reply.text = [
          "Plain words first.",
          '<script>window.hacked = true</script><img src="x" onerror="window.hacked = true">',
          '<iframe src="https://example.com"></iframe><style>body{display:none}</style>',
          '<a href="https://example.com" onclick="window.hacked = true">raw link</a>',
          "[a script link](javascript:alert(1)) and ![an image](https://example.com/x.png)",
          '<div onmouseover="window.hacked = true">hover</div>',
        ].join("\n\n");
        reply.citations = [];
        reply.sources = [];
      },
    });
    const thread = await openChat(user, panel, /slab pull/);

    expect(thread).toHaveTextContent("Plain words first.");
    expect(
      thread.querySelectorAll("script, img, iframe, style, object, embed"),
    ).toHaveLength(0);
    for (const element of thread.querySelectorAll("*")) {
      for (const attribute of element.getAttributeNames()) {
        expect(attribute.startsWith("on")).toBe(false);
      }
    }
    for (const link of thread.querySelectorAll("a")) {
      expect(link.getAttribute("href")).toMatch(/^https?:\/\//);
    }
    // The sections' Markdown drops the tags and keeps the words between them.
    const rawLink = within(thread).getByText("raw link");
    expect(rawLink).toBeVisible();
    expect(rawLink.closest("a")).toBeNull();
    expect(within(thread).queryByRole("link")).toBeNull();
    expect(within(thread).getByText("a script link")).toBeVisible();
    expect("hacked" in window).toBe(false);
  });

  it("All side chats goes back to the list with the chat just used first", async () => {
    const { panel, user } = await renderPanel();
    const thread = await startChat(user, panel);
    await user.type(
      within(thread).getByRole("textbox", { name: "Ask a question" }),
      "Why do continents not sink?{Enter}",
    );
    await within(thread).findByText(LAST_CHUNK, { exact: false });

    await user.click(
      within(panel).getByRole("button", { name: "All side chats" }),
    );
    const list = await within(panel).findByRole("list", { name: "Side chats" });
    const entries = within(list).getAllByRole("listitem");
    expect(entries).toHaveLength(2);
    expect(entries[0]).toHaveTextContent("Why do continents not sink?");
    expect(within(panel).queryByRole("region", { name: "Side chat" })).toBe(
      null,
    );
  });
});

describe("the prompt field", () => {
  it("sends on Enter, breaks the line on Shift+Enter, never sends empty text, and holds the model picker in its row", async () => {
    const { panel, requests, user } = await renderPanel();
    const thread = await startChat(user, panel);
    const field = within(panel).getByRole("textbox", {
      name: "Ask a question",
    });
    const sent = () =>
      requests.filter(({ type }) => type === "SendSideChatMessage");

    expect(within(panel).getByRole("button", { name: "Send" })).toBeDisabled();
    await user.type(field, "   {Enter}");
    expect(sent()).toHaveLength(0);

    await user.clear(field);
    await user.type(field, "First line{Shift>}{Enter}{/Shift}second line");
    expect(field).toHaveValue("First line\nsecond line");
    expect(sent()).toHaveLength(0);

    const row = field.closest("form")!;
    expect(
      within(row).getByRole("button", { name: /^Model: GPT-5\.5$/ }),
    ).toBeVisible();
    expect(within(row).queryByRole("progressbar")).toBeNull();
    expect(within(row).queryByRole("meter")).toBeNull();

    await user.type(field, "{Enter}");
    expect(sent()).toEqual([
      expect.objectContaining({ text: "First line\nsecond line" }),
    ]);
    expect(field).toHaveValue("");
    expect(
      await within(thread).findByText(LAST_CHUNK, { exact: false }),
    ).toBeVisible();
  });
});

describe("a reply as it is written", () => {
  it("shows a row while and after the web search, streams the text, and Stop marks the partial reply stopped", async () => {
    const valve = createValve();
    const { panel, requests, user, core } = await renderPanel({
      pace: valve.pace,
    });
    const thread = await openChat(user, panel, /slab pull/);
    const chatId = sampleChat(core.world).id;

    await user.type(
      within(panel).getByRole("textbox", { name: "Ask a follow-up" }),
      "What about ridge push?{Enter}",
    );
    expect(await within(thread).findByText("Searching the web…")).toBeVisible();
    expect(within(thread).getByText("What about ridge push?")).toBeVisible();
    expect(
      within(thread).queryByRole("button", { name: FOLLOW_UPS[0] }),
    ).toBeNull();
    const stop = within(panel).getByRole("button", { name: "Stop" });
    expect(within(panel).queryByRole("button", { name: "Send" })).toBeNull();

    await act(async () => valve.step());
    await act(async () => valve.step());
    // The stored reply holds the same text: the new reply adds a second copy
    // of its first sentence and none of its last.
    await waitFor(() =>
      expect(
        within(thread).getAllByText(FIRST_CHUNK, { exact: false }),
      ).toHaveLength(2),
    );
    expect(within(thread).getAllByText("Searched the web")).toHaveLength(2);
    expect(
      within(thread).getAllByText(LAST_CHUNK, { exact: false }),
    ).toHaveLength(1);

    await user.click(stop);
    expect(requests).toContainEqual({ type: "StopSideChatReply", chatId });
    await act(async () => valve.open());

    expect(
      await within(thread).findByText(
        "You stopped this reply before it was finished.",
      ),
    ).toBeVisible();
    expect(
      within(thread).getAllByText(LAST_CHUNK, { exact: false }),
    ).toHaveLength(1);
    expect(
      within(thread).getAllByText(FIRST_CHUNK, { exact: false }),
    ).toHaveLength(2);
    expect(within(panel).queryByRole("button", { name: "Stop" })).toBeNull();
    expect(within(panel).getByRole("button", { name: "Send" })).toBeVisible();
    expect(
      within(thread).queryByRole("button", { name: FOLLOW_UPS[0] }),
    ).toBeNull();
  });

  it("follows a streamed reply to its end and offers the follow-ups after it", async () => {
    const valve = createValve();
    const { panel, user } = await renderPanel({ pace: valve.pace });
    const thread = await startChat(user, panel);

    await user.type(
      within(panel).getByRole("textbox", { name: "Ask a question" }),
      "Why do plates move?{Enter}",
    );
    await act(async () => valve.step());
    await act(async () => valve.step());
    expect(
      await within(thread).findByText(FIRST_CHUNK, { exact: false }),
    ).toBeVisible();
    expect(
      within(thread).queryByRole("button", { name: FOLLOW_UPS[0] }),
    ).toBeNull();

    await act(async () => valve.open());
    expect(
      await within(thread).findByRole("button", { name: FOLLOW_UPS[0] }),
    ).toBeVisible();
    expect(thread).toHaveTextContent(LAST_CHUNK);
    expect(
      within(thread).getByRole("button", { name: SOURCE_1 }),
    ).toBeVisible();
  });

  it("a failed reply says why and sends the question again", async () => {
    const { panel, requests, user, core } = await renderPanel({
      setup: (world) => {
        world.fault = {
          type: "Fail",
          error: { type: "FailedAfterRetries", attempts: 3 },
          retries: 0,
        };
      },
    });
    const thread = await startChat(user, panel);
    await user.type(
      within(panel).getByRole("textbox", { name: "Ask a question" }),
      "Why do plates move?{Enter}",
    );

    expect(
      await within(thread).findByText(
        "This reply could not be written: the request failed after 3 attempts.",
      ),
    ).toBeVisible();
    expect(within(thread).getByText("This reply is incomplete.")).toBeVisible();

    core.world.fault = null;
    await user.click(
      within(thread).getByRole("button", { name: "Send again" }),
    );
    expect(
      requests.filter(({ type }) => type === "SendSideChatMessage"),
    ).toEqual([
      expect.objectContaining({ text: "Why do plates move?" }),
      expect.objectContaining({ text: "Why do plates move?" }),
    ]);
    expect(
      await within(thread).findByText(LAST_CHUNK, { exact: false }),
    ).toBeVisible();
    expect(within(thread).getAllByText("Why do plates move?")).toHaveLength(1);
    expect(
      within(thread).queryByRole("button", { name: "Send again" }),
    ).toBeNull();
  });

  it("at the plan's limit it says so, when it resets, and offers Manage usage", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(startedAt));
    const { panel, requests, user } = await renderPanel({
      scenario: "plan-limit",
    });
    const thread = await startChat(user, panel);
    await user.type(
      within(panel).getByRole("textbox", { name: "Ask a question" }),
      "Why do plates move?{Enter}",
    );

    expect(
      await within(thread).findByText("The plan's usage limit is reached."),
    ).toBeVisible();
    expect(within(thread).getByText(/It resets in 2 h 14 min\./)).toBeVisible();
    await user.click(
      within(thread).getByRole("button", { name: "Manage usage" }),
    );
    expect(requests).toContainEqual({ type: "OpenManageUsage" });
  });

  it("offline, a stored chat stays readable and a question says the app is offline", async () => {
    const { panel, user } = await renderPanel({ scenario: "offline" });
    const thread = await openChat(user, panel, /slab pull/);
    expect(thread).toHaveTextContent(FIRST_CHUNK);

    await user.type(
      within(panel).getByRole("textbox", { name: "Ask a follow-up" }),
      "What about ridge push?{Enter}",
    );

    expect(
      await within(thread).findByText(/^The app is offline\./),
    ).toBeVisible();
    expect(within(thread).getByText("What about ridge push?")).toBeVisible();
    expect(
      within(thread).getByRole("button", { name: "Send again" }),
    ).toBeVisible();
  });

  it("a citation chip opens its source through the core", async () => {
    const { panel, requests, user } = await renderPanel();
    const thread = await openChat(user, panel, /slab pull/);

    await user.click(within(thread).getByRole("button", { name: SOURCE_1 }));

    expect(requests).toContainEqual({
      type: "OpenExternal",
      url: "https://doi.org/10.1111/j.1365-246X.1975.tb00631.x",
    });
  });
});
