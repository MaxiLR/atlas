import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  AtlasStoreProvider,
  failureInStore,
  useAtlasStore,
  type AtlasStore,
} from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import { noticeShown, type Notice } from "@/store/notices";
import { openCourse, startedAt } from "@/test/fake-core";
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
