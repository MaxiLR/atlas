import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { useEffect } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App } from "@/App";

import {
  AtlasStoreProvider,
  failureInStore,
  useAtlasStore,
  type AtlasStore,
} from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock, type FakeWorld } from "@/core/fake";
import { noticeShown, type Notice } from "@/store/notices";
import { drained, openCourse, startedAt } from "@/test/fake-core";
import { ThemeProvider } from "@/theme/provider";

import { Notices } from "./notices";

/** Sonner closes a notice without a duration of its own after 4 s. */
const SONNER_DEFAULT_DURATION = 4000;

const saved: Notice = {
  id: "pdf",
  kind: "success",
  title: "PDF saved",
  description: "Plate tectonics.pdf",
  action: null,
};

function Store({ onStore }: { onStore: (store: AtlasStore) => void }) {
  const store = useAtlasStore();
  useEffect(() => {
    onStore(store);
  }, [store, onStore]);
  return null;
}

/**
 * The notices over the default scenario's core, except that the core cannot
 * read the session: its secret store fails.
 */
function renderNotices() {
  const core = createFakeCore({
    scenario: "default",
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  const client: CoreClient = {
    call(request) {
      return request.type === "GetSession"
        ? Promise.reject({
            type: "Store",
            message: "secrets.json is not valid JSON.",
          })
        : core.call(request);
    },
    subscribe: (listener) => core.subscribe(listener),
  };
  let store: AtlasStore | null = null;
  render(
    <ThemeProvider>
      <CoreProvider client={client}>
        <AtlasStoreProvider>
          <Store onStore={(given) => (store = given)} />
          <Notices />
        </AtlasStoreProvider>
      </CoreProvider>
    </ThemeProvider>,
  );
  return store!;
}

/** The session could not be read: the store shows its notice. */
async function failSession(store: AtlasStore) {
  await act(() => store.send({ type: "GetSession" }).catch(failureInStore));
}

/** Puts a notice in the store as a later outcome would. */
function show(store: AtlasStore, notice: Notice) {
  act(() =>
    store.state.setState(
      (state) => ({ ...state, notices: noticeShown(state.notices, notice) }),
      true,
    ),
  );
}

const elapse = (ms: number) => act(() => vi.advanceTimersByTime(ms));

/** Sonner's clock: the timeouts and frames it schedules, and the time. */
function fakeSonnerClock() {
  vi.useFakeTimers({
    toFake: [
      "setTimeout",
      "clearTimeout",
      "requestAnimationFrame",
      "cancelAnimationFrame",
      "Date",
    ],
  });
}

/** The list Sonner places the notices in. */
function toaster() {
  return within(
    screen.getByRole("region", { name: /^Notifications/ }),
  ).getByRole("list");
}

afterEach(() => {
  vi.useRealTimers();
});

describe("notices", () => {
  it("an error is an alert with Dismiss, and outlasts Sonner's default duration", async () => {
    fakeSonnerClock();
    const store = renderNotices();
    await failSession(store);
    elapse(50);

    const notice = screen.getByRole("alert");
    expect(notice).toHaveTextContent("Session could not be read");
    expect(notice).toHaveTextContent("secrets.json is not valid JSON.");
    expect(
      within(notice).getByRole("button", { name: "Dismiss" }),
    ).toBeInTheDocument();

    elapse(SONNER_DEFAULT_DURATION * 3);

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Session could not be read",
    );
    expect(store.state.getState().notices).toHaveLength(1);
  });

  it("a success put in the store is a status that closes on its own", () => {
    fakeSonnerClock();
    const store = renderNotices();
    show(store, saved);
    elapse(50);

    const notice = screen.getByRole("status");
    expect(notice).toHaveTextContent("PDF saved");
    expect(notice).toHaveTextContent("Plate tectonics.pdf");
    expect(
      within(notice).getByRole("button", { name: "Dismiss" }),
    ).toBeInTheDocument();

    elapse(SONNER_DEFAULT_DURATION + 1000);

    expect(screen.queryByText("PDF saved")).toBeNull();
    expect(store.state.getState().notices).toEqual([]);
  });

  it("a success stays while focused, and closes on its own once left", () => {
    fakeSonnerClock();
    const store = renderNotices();
    show(store, saved);
    elapse(50);
    const dismiss = within(screen.getByRole("status")).getByRole("button", {
      name: "Dismiss",
    });

    act(() => dismiss.focus());
    elapse(SONNER_DEFAULT_DURATION * 3);
    expect(screen.getByRole("status")).toHaveTextContent("PDF saved");

    act(() => dismiss.blur());
    elapse(SONNER_DEFAULT_DURATION + 1000);
    expect(screen.queryByText("PDF saved")).toBeNull();
    expect(store.state.getState().notices).toEqual([]);
  });

  it("Dismiss closes the notice and takes it out of the store", async () => {
    const store = renderNotices();
    await failSession(store);
    const user = userEvent.setup();
    const notice = await screen.findByRole("alert");

    await user.click(within(notice).getByRole("button", { name: "Dismiss" }));

    expect(store.state.getState().notices).toEqual([]);
    await expect.poll(() => screen.queryByRole("alert")).toBeNull();
  });

  it("a notice taken out of the store leaves the window", async () => {
    const store = renderNotices();
    await failSession(store);
    await screen.findByRole("alert");

    act(() => store.navigate({ type: "DismissNotice", id: "session" }));

    await expect.poll(() => screen.queryByRole("alert")).toBeNull();
  });

  it("sits 16 px from the right and bottom, and above the foot in the reader", async () => {
    const store = renderNotices();
    await failSession(store);
    await screen.findByRole("alert");

    expect(toaster().style.getPropertyValue("--offset-right")).toBe("16px");
    expect(toaster().style.getPropertyValue("--offset-bottom")).toBe("16px");

    await act(() => openCourse(store, "Plate tectonics"));

    expect(store.state.getState().navigation.screen).toBe("reader");
    expect(toaster().style.getPropertyValue("--offset-bottom")).toBe("80px");
    expect(toaster().style.getPropertyValue("--offset-right")).toBe("16px");
  });
});

const BROWSER = "No application is set to open web links.";
const LOCKED = "The database is locked by another process.";

/**
 * The app over the fake core in `scenario`, after `setup` has set the
 * world's conditions.
 */
function renderApp(
  scenario: string,
  setup: (world: FakeWorld) => void = () => {},
) {
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  setup(core.world);
  render(
    <CoreProvider client={core}>
      <App />
    </CoreProvider>,
  );
  return { core, user: userEvent.setup() };
}

/** Plate tectonics, as the fake core's world holds it. */
function plateTectonics(world: FakeWorld) {
  return world.courses.find(({ topic }) => topic === "Plate tectonics")!;
}

/** Opens Plate tectonics from the library, at Convergent boundaries. */
async function openReader(user: UserEvent) {
  await user.click(
    await screen.findByRole("button", { name: "Plate tectonics" }),
  );
  return screen.findByRole("article", { name: "Convergent boundaries" });
}

/** Opens the side panel on the tab whose reader button is `name`. */
async function openPanel(user: UserEvent, name: string) {
  await user.click(screen.getByRole("button", { name }));
  return screen.getByRole("complementary", { name: "Side panel" });
}

/** The error notice that appears, from Sonner's list of notices. */
function findErrorNotice() {
  return within(
    screen.getByRole("region", { name: /^Notifications/ }),
  ).findByRole("alert");
}

/** Once the core has run as far as it can, no notice is showing. */
async function expectNoNotice() {
  await act(drained);
  const notifications = screen.getByRole("region", {
    name: /^Notifications/,
  });
  expect(within(notifications).queryByRole("alert")).toBeNull();
  expect(within(notifications).queryByRole("status")).toBeNull();
}

describe("a link that could not be opened", () => {
  it("from the glossary's Wikipedia link, says so with the reason", async () => {
    const { user } = renderApp("default", (world) => {
      world.browserFailure = BROWSER;
      plateTectonics(world).glossary[0]!.wikipedia = {
        type: "Found",
        title: "Asthenosphere",
        extract: "The asthenosphere is the weak region of the upper mantle.",
        url: "https://en.wikipedia.org/wiki/Asthenosphere",
      };
    });
    await openReader(user);
    const panel = await openPanel(user, "Glossary 1");

    await user.click(
      await within(panel).findByRole("link", {
        name: "Wikipedia article on asthenosphere",
      }),
    );

    const notice = await findErrorNotice();
    expect(notice).toHaveTextContent("Link could not be opened");
    expect(notice).toHaveTextContent(BROWSER);
  });

  it("from a citation, says so with the reason", async () => {
    const { user } = renderApp("default", (world) => {
      world.browserFailure = BROWSER;
    });
    const article = await openReader(user);

    await user.click(
      within(article).getAllByRole("button", { name: /^Source 1: / })[0]!,
    );

    const notice = await findErrorNotice();
    expect(notice).toHaveTextContent("Link could not be opened");
    expect(notice).toHaveTextContent(BROWSER);
  });

  it("from a source, says so with the reason", async () => {
    const { user } = renderApp("default", (world) => {
      world.browserFailure = BROWSER;
    });
    const article = await openReader(user);

    await user.click(
      within(within(article).getByRole("list", { name: "Sources" })).getByRole(
        "link",
        { name: /Subduction zones/ },
      ),
    );

    const notice = await findErrorNotice();
    expect(notice).toHaveTextContent("Link could not be opened");
    expect(notice).toHaveTextContent(BROWSER);
  });

  it("from Manage usage in a section's status, says so with the reason", async () => {
    const { user } = renderApp("plan-limit", (world) => {
      world.browserFailure = BROWSER;
    });
    await openReader(user);
    const index = screen.getByRole("navigation", { name: "Index" });
    await user.click(
      within(index).getByRole("button", { name: "Transform boundaries" }),
    );
    const article = await screen.findByRole("article", {
      name: "Transform boundaries",
    });

    await user.click(
      await within(article).findByRole("button", { name: "Manage usage" }),
    );

    const notice = await findErrorNotice();
    expect(notice).toHaveTextContent("Link could not be opened");
    expect(notice).toHaveTextContent(BROWSER);
  });

  it("from Manage usage in a side-chat reply, says so with the reason", async () => {
    const { user } = renderApp("plan-limit", (world) => {
      world.browserFailure = BROWSER;
    });
    await openReader(user);
    const panel = await openPanel(user, "Side chats 1");
    await within(panel).findByRole("list", { name: "Side chats" });
    await user.click(
      within(panel).getByRole("button", { name: "New side chat" }),
    );
    const thread = await within(panel).findByRole("region", {
      name: "Side chat",
    });
    await user.type(
      within(panel).getByRole("textbox", { name: "Ask a question" }),
      "Why do plates move?{Enter}",
    );

    await user.click(
      await within(thread).findByRole("button", { name: "Manage usage" }),
    );

    const notice = await findErrorNotice();
    expect(notice).toHaveTextContent("Link could not be opened");
    expect(notice).toHaveTextContent(BROWSER);
  });
});

describe("a delete that failed", () => {
  it("of a definition, names it with the reason, and the row stays", async () => {
    const { user } = renderApp("default", (world) => {
      world.deleteFailure = LOCKED;
    });
    await openReader(user);
    const panel = await openPanel(user, "Glossary 1");
    const list = await within(panel).findByRole("list", { name: "Glossary" });

    await user.click(
      within(list).getByRole("button", { name: "Glossary entry menu" }),
    );
    await user.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", {
        name: "Delete definition",
      }),
    );

    const notice = await findErrorNotice();
    expect(notice).toHaveTextContent("Definition could not be deleted");
    expect(notice).toHaveTextContent(LOCKED);
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);
    expect(within(list).getByRole("term")).toHaveTextContent("asthenosphere");
  });

  it("of a side chat, names it with the reason, and the row stays", async () => {
    const { user } = renderApp("default", (world) => {
      world.deleteFailure = LOCKED;
    });
    await openReader(user);
    const panel = await openPanel(user, "Side chats 1");
    const list = await within(panel).findByRole("list", {
      name: "Side chats",
    });

    await user.click(
      within(list).getByRole("button", { name: "Side chat menu" }),
    );
    await user.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", {
        name: "Delete side chat",
      }),
    );

    const notice = await findErrorNotice();
    expect(notice).toHaveTextContent("Side chat could not be deleted");
    expect(notice).toHaveTextContent(LOCKED);
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);
    expect(within(list).getByRole("listitem")).toHaveTextContent(/slab pull/);
  });
});

describe("outcomes shown where they happen", () => {
  it("deleting a definition or a side chat shows no notice", async () => {
    const { user } = renderApp("default");
    await openReader(user);
    const panel = await openPanel(user, "Glossary 1");
    const glossary = await within(panel).findByRole("list", {
      name: "Glossary",
    });
    await user.click(
      within(glossary).getByRole("button", { name: "Glossary entry menu" }),
    );
    await user.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", {
        name: "Delete definition",
      }),
    );
    await within(panel).findByText(
      "No terms yet: the terms you define while reading appear here.",
    );

    await user.click(within(panel).getByRole("tab", { name: "Side chats 1" }));
    const chats = await within(panel).findByRole("list", {
      name: "Side chats",
    });
    await user.click(
      within(chats).getByRole("button", { name: "Side chat menu" }),
    );
    await user.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", {
        name: "Delete side chat",
      }),
    );
    await waitFor(() =>
      expect(
        within(panel).queryByRole("list", { name: "Side chats" }),
      ).toBeNull(),
    );

    await expectNoNotice();
  });

  it("signing out shows no notice", async () => {
    const { user } = renderApp("default");
    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(
      within(screen.getByRole("region", { name: "Account" })).getByRole(
        "button",
        { name: "Sign out" },
      ),
    );

    expect(
      await screen.findByRole("button", { name: "Continue with ChatGPT" }),
    ).toBeVisible();
    await expectNoNotice();
  });

  it("deleting a course shows no notice", async () => {
    const { user } = renderApp("default");
    const row = (
      await screen.findByRole("button", { name: "Plate tectonics" })
    ).closest("li")!;
    await user.click(within(row).getByRole("button", { name: "Course menu" }));
    await user.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", {
        name: "Delete course",
      }),
    );
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: "Delete course",
      }),
    );

    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Plate tectonics" }),
      ).toBeNull(),
    );
    await expectNoNotice();
  });
});

describe("the failure notices", () => {
  it("stay until dismissed, each an alert with Dismiss", async () => {
    fakeSonnerClock();
    const core = createFakeCore({
      scenario: "default",
      clock: createManualClock(startedAt),
      pace: () => Promise.resolve(),
    });
    core.world.browserFailure = BROWSER;
    core.world.deleteFailure = LOCKED;
    const entryId = plateTectonics(core.world).glossary[0]!.id;
    let store: AtlasStore | null = null;
    render(
      <ThemeProvider>
        <CoreProvider client={core}>
          <AtlasStoreProvider>
            <Store onStore={(given) => (store = given)} />
            <Notices />
          </AtlasStoreProvider>
        </CoreProvider>
      </ThemeProvider>,
    );
    await act(() =>
      Promise.all([
        store!
          .send({
            type: "OpenExternal",
            url: "https://en.wikipedia.org/wiki/Asthenosphere",
          })
          .catch(failureInStore),
        store!
          .send({ type: "DeleteGlossaryEntry", entryId })
          .catch(failureInStore),
      ]),
    );
    elapse(50);
    expect(screen.getAllByRole("alert")).toHaveLength(2);

    elapse(SONNER_DEFAULT_DURATION * 3);

    const notices = screen.getAllByRole("alert");
    expect(notices.map((notice) => notice.textContent)).toEqual([
      expect.stringContaining("Link could not be opened"),
      expect.stringContaining("Definition could not be deleted"),
    ]);
    act(() =>
      within(notices[0]!).getByRole("button", { name: "Dismiss" }).click(),
    );
    expect(
      store!.state.getState().notices.map((notice) => notice.title),
    ).toEqual(["Definition could not be deleted"]);
  });
});
