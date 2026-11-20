import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  AtlasStoreProvider,
  useAtlasStore,
  type AtlasStore,
} from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CoreResponse } from "@/core/generated/CoreResponse";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { SectionView } from "@/core/generated/SectionView";
import type { UsageWindow } from "@/core/generated/UsageWindow";

import { SectionStatus } from "./section-status";

const courseId = "course-1";
const sectionId = "section-1";
const ids = { courseId, sectionId };

function Store({ onStore }: { onStore: (store: AtlasStore) => void }) {
  const store = useAtlasStore();
  useEffect(() => {
    onStore(store);
  }, [store, onStore]);
  return null;
}

/**
 * The status of one section over a store fed by a scripted core: `answer`
 * answers each request (never, by default), `emit` and `generate` send the
 * core's events, `open` sends OpenSection the way the reading column does.
 * `requests` records what the status asked of the core.
 */
function renderStatus(
  answer: (request: CoreRequest) => Promise<CoreResponse> = () =>
    new Promise(() => {}),
) {
  const requests: CoreRequest[] = [];
  const listeners = new Set<(event: CoreEvent) => void>();
  const client: CoreClient = {
    call(request) {
      requests.push(request);
      return answer(request) as never;
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  let store: AtlasStore | null = null;
  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <Store onStore={(given) => (store = given)} />
        <SectionStatus courseId={courseId} sectionId={sectionId} />
      </AtlasStoreProvider>
    </CoreProvider>,
  );
  const emit = (...events: CoreEvent[]) =>
    act(() => {
      for (const event of events) {
        for (const listener of listeners) listener(event);
      }
    });
  const generate = (...events: GenerationEvent[]) =>
    emit(
      ...events.map((event): CoreEvent => ({
        type: "Generation",
        generationId: "gen-1",
        target: { type: "Section", courseId, sectionId, style: "Original" },
        event,
      })),
    );
  const open = () =>
    act(async () => {
      await store!
        .send({ type: "OpenSection", courseId, sectionId })
        .catch(() => {});
    });
  return {
    requests,
    emit,
    generate,
    open,
    user: userEvent.setup(),
  };
}

const refuse = (error: CoreError) => (request: CoreRequest) =>
  request.type === "OpenSection"
    ? Promise.reject(error)
    : Promise.resolve<CoreResponse>({ type: "Done" });

function stored(writtenWithoutSearch: boolean): SectionView {
  return {
    id: sectionId,
    courseId,
    title: "Transform boundaries",
    summary: "Where plates slide past each other.",
    version: {
      id: "version-1",
      style: "Original",
      blocks: [
        {
          index: 0,
          content: { type: "Prose", text: "Plates slide past each other." },
        },
      ],
      citations: [],
      sources: [],
      writtenWithoutSearch,
      model: "gpt-5.5",
      createdAt: "2026-11-18T09:00:00.000Z",
    },
    versions: [
      {
        id: "version-1",
        style: "Original",
        createdAt: "2026-11-18T09:00:00.000Z",
      },
    ],
  };
}

const partial: GenerationEvent[] = [
  { type: "Started" },
  { type: "Activity", activity: { type: "Writing" } },
  { type: "BlockOpened", block: 0, kind: "Prose" },
  { type: "TextDelta", block: 0, text: "Plates slide " },
];

afterEach(() => {
  vi.useRealTimers();
});

describe("a section being generated", () => {
  it("says what the agent is doing as it happens, in a polite status, with Stop, without taking focus", async () => {
    const { generate, requests, user } = renderStatus();

    generate(
      { type: "Started" },
      { type: "Activity", activity: { type: "Searching" } },
    );
    expect(screen.getByRole("status")).toHaveTextContent("Searching the web");
    expect(document.body).toHaveFocus();

    generate({ type: "Activity", activity: { type: "Writing" } });
    expect(screen.getByRole("status")).toHaveTextContent(
      "Writing this section",
    );

    generate({ type: "Activity", activity: { type: "Retrying", attempt: 3 } });
    expect(screen.getByRole("status")).toHaveTextContent("Retrying, attempt 3");
    expect(document.body).toHaveFocus();

    await user.click(screen.getByRole("button", { name: "Stop" }));
    expect(requests).toContainEqual({ type: "StopGeneration", ...ids });
  });

  it("shows the web search it ran with the number of sources found", () => {
    const { generate } = renderStatus();

    generate(
      { type: "Started" },
      { type: "Activity", activity: { type: "Searching" } },
      { type: "Activity", activity: { type: "Writing" } },
      { type: "BlockOpened", block: 0, kind: "Prose" },
      {
        type: "CitationAdded",
        citation: { block: 0, start: 0, end: 6, source: 1 },
        source: {
          number: 1,
          url: "https://example.org/plates",
          title: "Plates",
        },
      },
      {
        type: "CitationAdded",
        citation: { block: 0, start: 7, end: 12, source: 2 },
        source: {
          number: 2,
          url: "https://example.org/faults",
          title: "Faults",
        },
      },
    );

    expect(screen.getByText("Searched the web")).toBeVisible();
    expect(screen.getByText("2 sources")).toBeVisible();
  });
});

describe("a stopped or failed section", () => {
  it("is marked incomplete with Retry once stopped, which sends RetrySection", async () => {
    const { generate, open, requests, user } = renderStatus(
      refuse({ type: "Provider", message: "the generation was stopped" }),
    );

    await open();
    generate(...partial, { type: "Finished", outcome: { type: "Stopped" } });

    expect(screen.getByText("This section is incomplete.")).toBeVisible();
    expect(
      screen.getByText("You stopped it before it was finished."),
    ).toBeVisible();
    expect(screen.queryByText(/the generation was stopped/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(requests).toContainEqual({ type: "RetrySection", ...ids });
  });

  it("is shown as failed with Retry after three retries, naming the attempts", async () => {
    const error: CoreError = { type: "FailedAfterRetries", attempts: 4 };
    const { generate, open, requests, user } = renderStatus(refuse(error));

    await open();
    generate(
      { type: "Started" },
      { type: "Activity", activity: { type: "Writing" } },
      { type: "Activity", activity: { type: "Retrying", attempt: 2 } },
      { type: "Activity", activity: { type: "Retrying", attempt: 3 } },
      { type: "Activity", activity: { type: "Retrying", attempt: 4 } },
      { type: "Finished", outcome: { type: "Failed", error } },
    );

    expect(
      screen.getByText(
        "This section could not be written: the request failed after 4 attempts.",
      ),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(requests).toContainEqual({ type: "RetrySection", ...ids });
  });
});

describe("the plan's limit reached", () => {
  const now = new Date("2026-11-19T10:00:00.000Z");
  const limit = (resetsAt: string | null): CoreError => ({
    type: "PlanLimit",
    resetsAt,
  });

  it("says so, shows when it resets and offers Manage usage first; nothing retries by itself", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(now);
    const error = limit("2026-11-19T12:14:00.000Z");
    const { generate, open, requests, user } = renderStatus(refuse(error));

    await open();
    generate(...partial, {
      type: "Finished",
      outcome: { type: "Failed", error },
    });

    expect(
      screen.getByText("The plan's usage limit is reached."),
    ).toBeVisible();
    expect(screen.getByText("It resets in 2 h 14 min.")).toBeVisible();
    const buttons = screen.getAllByRole("button");
    expect(buttons[0]).toHaveAccessibleName("Manage usage");
    expect(requests).toEqual([{ type: "OpenSection", ...ids }]);

    await user.click(buttons[0]);
    expect(requests).toContainEqual({ type: "OpenManageUsage" });
    expect(
      requests.filter((request) => request.type === "RetrySection"),
    ).toEqual([]);
  });

  it("shows no reset time when nothing knows it", async () => {
    const { open } = renderStatus(refuse(limit(null)));

    await open();

    expect(
      screen.getByText("The plan's usage limit is reached."),
    ).toBeVisible();
    expect(screen.queryByText(/resets/)).toBeNull();
    expect(screen.getByRole("button", { name: "Manage usage" })).toBeVisible();
  });

  it("takes the reset time from the usage meter when the refusal has none", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(now);
    const window = (
      name: string,
      used: number,
      resetsAt: string,
    ): UsageWindow => ({
      name,
      lengthSeconds: name === "5h" ? 5 * 3600 : 7 * 86400,
      used,
      resetsAt,
      level: used >= 1 ? "Limit" : "Normal",
    });
    const { emit, open } = renderStatus(refuse(limit(null)));

    emit({
      type: "UsageChanged",
      usage: {
        type: "Connected",
        reading: {
          windows: [
            window("5h", 1, "2026-11-19T10:45:00.000Z"),
            window("7d", 0.5, "2026-11-23T09:00:00.000Z"),
          ],
          takenAt: now.toISOString(),
          stale: null,
        },
      },
    });
    await open();

    expect(screen.getByText("It resets in 45 min.")).toBeVisible();
  });

  it("names the day and time of a reset more than a day away", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(2026, 10, 19, 10, 0));
    const { open } = renderStatus(
      refuse(limit(new Date(2026, 10, 23, 9, 0).toISOString())),
    );

    await open();

    expect(screen.getByText("It resets Mon 09:00.")).toBeVisible();
  });
});

describe("plan usage not allowed", () => {
  it("explains it once, with no sign-in and nothing to retry", async () => {
    const { open } = renderStatus(refuse({ type: "PlanNotAllowed" }));

    await open();

    expect(
      screen.getByText(
        "Plan usage is not allowed for this account or workspace, so Atlas cannot write new sections. Sections already written stay readable.",
      ),
    ).toBeVisible();
    expect(screen.queryAllByRole("button")).toEqual([]);
    expect(screen.queryByText(/sign in/i)).toBeNull();
  });
});

describe("offline", () => {
  it("says the app is offline for a section that needs a request, with Retry", async () => {
    const { open, requests, user } = renderStatus(refuse({ type: "Offline" }));

    await open();

    expect(
      screen.getByText(
        "The app is offline. This section is written once it is back online.",
      ),
    ).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(requests).toContainEqual({ type: "RetrySection", ...ids });
  });

  it("shows a stored section normally", async () => {
    const { open } = renderStatus(() =>
      Promise.resolve({ type: "Section", section: stored(false) }),
    );

    await open();

    expect(screen.queryByText(/offline/i)).toBeNull();
    expect(screen.queryByText(/incomplete/i)).toBeNull();
    expect(screen.queryAllByRole("button")).toEqual([]);
  });
});

describe("a section written without web search", () => {
  it("says so in one line", async () => {
    const { open } = renderStatus(() =>
      Promise.resolve({ type: "Section", section: stored(true) }),
    );

    await open();

    expect(
      screen.getByText(
        "Written without web search: the account's policy does not allow it.",
      ),
    ).toBeVisible();
    expect(screen.queryAllByRole("button")).toEqual([]);
  });
});
