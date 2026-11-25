import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AtlasStoreProvider } from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CoreResponse } from "@/core/generated/CoreResponse";
import type { UsageFailure } from "@/core/generated/UsageFailure";
import type { UsageLevel } from "@/core/generated/UsageLevel";
import type { UsageState } from "@/core/generated/UsageState";
import type { UsageWindow } from "@/core/generated/UsageWindow";

import { UsageMeter } from "./usage-meter";

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const NOW = Date.parse("2026-11-19T10:00:00.000Z");

/** A window of `length` ms, `used` of it spent, resetting `resetsIn` from now. */
function win(
  name: string,
  length: number,
  used: number,
  resetsIn: number | null,
  level: UsageLevel = "Normal",
): UsageWindow {
  return {
    name,
    lengthSeconds: length / SECOND,
    used,
    resetsAt: resetsIn === null ? null : new Date(NOW + resetsIn).toISOString(),
    level,
  };
}

// 2h 45m of 5h passed: 55 %. 79h of 168h passed: 47 %. 13 of 30 days: 43 %.
const fiveHours = win("5h", 5 * HOUR, 0.38, 2 * HOUR + 15 * MINUTE);
const weekly = win("7d", 7 * DAY, 0.61, 3 * DAY + 17 * HOUR);
const monthly = win("30d", 30 * DAY, 0.12, 17 * DAY);

function connected(
  windows: UsageWindow[],
  stale: UsageFailure | null = null,
): UsageState {
  return {
    type: "Connected",
    reading: { windows, takenAt: new Date(NOW).toISOString(), stale },
  };
}

/**
 * Mounts the meter over a core that reports `usage` and waits until the
 * meter has that answer; a null `usage` is a core that has not answered yet.
 * `requests` records what the meter asked; `emit` sends the meter an event
 * from the core.
 */
async function mountMeter(usage: UsageState | null, folded = false) {
  const requests: CoreRequest[] = [];
  const listeners = new Set<(event: CoreEvent) => void>();
  const client: CoreClient = {
    call(request: CoreRequest) {
      requests.push(request);
      if (usage === null) return new Promise<CoreResponse>(() => {});
      const response: CoreResponse = { type: "Usage", usage };
      return Promise.resolve(response);
    },
    subscribe(listener: (event: CoreEvent) => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  } as CoreClient;
  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <UsageMeter folded={folded} />
      </AtlasStoreProvider>
    </CoreProvider>,
  );
  // The answer to the meter's GetUsage reaches the store in a later task.
  await act(async () => {
    const { promise, resolve } = Promise.withResolvers<void>();
    setTimeout(resolve, 0);
    await promise;
  });
  const emit = (event: CoreEvent) =>
    act(() => {
      for (const listener of listeners) listener(event);
    });
  return { requests, emit, user: userEvent.setup() };
}

/** The mounted meter in a state that shows it; `meter` is its region. */
async function renderMeter(usage: UsageState, folded = false) {
  const mounted = await mountMeter(usage, folded);
  const meter = await screen.findByRole("region", { name: "Plan usage" });
  return { meter, ...mounted };
}

const signedIn: CoreEvent = {
  type: "SessionChanged",
  session: {
    type: "SignedIn",
    account: { email: "candidate@example.com" },
    planUsage: "On",
  },
};

/**
 * Two bars with no fill and no pace mark, hidden from assistive technology,
 * a status saying Reading usage, and nothing to press.
 */
function expectEmptyBars(meter: HTMLElement) {
  expect(within(meter).queryAllByRole("img")).toHaveLength(0);
  const bars = within(meter).getAllByRole("img", { hidden: true });
  expect(bars).toHaveLength(2);
  for (const bar of bars) expect(bar).toBeEmptyDOMElement();
  expect(within(meter).getByRole("status")).toHaveTextContent("Reading usage");
  expect(within(meter).queryAllByRole("button")).toHaveLength(0);
}

/**
 * The wide meter waiting for its first reading: the 5-hour and Weekly rows
 * in place but hidden from assistive technology, their figures a placeholder
 * with no text, their bars empty.
 */
function expectPendingRows(meter: HTMLElement) {
  expect(within(meter).queryAllByRole("term")).toHaveLength(0);
  expect(
    within(meter)
      .getAllByRole("term", { hidden: true })
      .map((term) => term.textContent),
  ).toEqual(["5-hour", "Weekly"]);
  const figures = within(meter).getAllByRole("definition", { hidden: true });
  expect(figures).toHaveLength(2);
  for (const figure of figures) {
    expect(figure).toHaveTextContent(/^$/);
    expect(figure).not.toBeEmptyDOMElement();
  }
  expectEmptyBars(meter);
}

/** Every state of the meter, by its tag. */
const states: [string, UsageState][] = [
  ["NoPlanLogin", { type: "NoPlanLogin" }],
  ["AwaitingReading", { type: "AwaitingReading" }],
  ["Connected", connected([fiveHours, weekly, monthly])],
  ["FirstReadFailed", { type: "FirstReadFailed", reason: "Auth" }],
];

/**
 * The meter reporting `usage` has no usage login to start, and still shows
 * a reading the core sends afterwards.
 */
async function expectNoConnectUsage(usage: UsageState, folded: boolean) {
  const { emit } = await mountMeter(usage, folded);

  expect(screen.queryByRole("button", { name: "Connect usage" })).toBeNull();
  expect(screen.queryByRole("dialog")).toBeNull();

  await emit({ type: "UsageChanged", usage: connected([weekly]) });

  expect(
    screen.getByRole("img", {
      name: "Weekly window, 61% used, 47% of the window elapsed",
    }),
  ).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Connect usage" })).toBeNull();
}

const elapse = (ms: number) => act(() => vi.advanceTimersByTime(ms));

beforeEach(() => {
  vi.useFakeTimers({
    toFake: ["Date", "setInterval", "clearInterval"],
    now: NOW,
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("the usage meter in every state", () => {
  it.each(states)("offers no Connect usage in %s", async (_tag, usage) => {
    await expectNoConnectUsage(usage, false);
  });
});

describe("the usage meter without a plan login", () => {
  it("shows nothing, and the bars once a reading arrives", async () => {
    const { requests, emit } = await mountMeter({ type: "NoPlanLogin" });

    expect(requests).toEqual([{ type: "GetUsage" }]);
    expect(screen.queryByRole("region", { name: "Plan usage" })).toBeNull();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(screen.queryAllByRole("img")).toHaveLength(0);

    await emit({ type: "UsageChanged", usage: connected([fiveHours, weekly]) });

    const meter = screen.getByRole("region", { name: "Plan usage" });
    expect(within(meter).getAllByRole("img")).toHaveLength(2);
  });

  it("goes away when the plan login ends, and comes back with the next one", async () => {
    const { emit } = await renderMeter(connected([fiveHours, weekly]));

    await emit({ type: "UsageChanged", usage: { type: "NoPlanLogin" } });
    expect(screen.queryByRole("region", { name: "Plan usage" })).toBeNull();
    expect(screen.queryAllByRole("img")).toHaveLength(0);

    await emit({ type: "UsageChanged", usage: { type: "AwaitingReading" } });
    expect(
      within(screen.getByRole("region", { name: "Plan usage" })).getByRole(
        "status",
      ),
    ).toHaveTextContent("Reading usage");
  });
});

describe("the usage meter, awaiting its first reading", () => {
  it("announces Reading usage, with nothing to press", async () => {
    const { meter } = await renderMeter({ type: "AwaitingReading" });

    expect(within(meter).getByRole("status")).toHaveTextContent(
      "Reading usage",
    );
    expect(within(meter).queryAllByRole("button")).toHaveLength(0);
  });

  it("keeps the 5-hour and Weekly rows in place, figures as a skeleton and bars empty", async () => {
    const { meter } = await renderMeter({ type: "AwaitingReading" });

    expectPendingRows(meter);
  });

  it("shows the bars once the reading arrives", async () => {
    const { meter, emit } = await renderMeter({ type: "AwaitingReading" });

    await emit({ type: "UsageChanged", usage: connected([fiveHours, weekly]) });

    expect(within(meter).queryByText("Reading usage")).toBeNull();
    expect(
      within(meter).getByRole("img", {
        name: "5-hour window, 38% used, 55% of the window elapsed",
      }),
    ).toBeInTheDocument();
  });
});

describe("the usage meter before the core has answered", () => {
  it("shows the pending rows while the session is signed in", async () => {
    const { emit, requests } = await mountMeter(null);
    expect(requests).toEqual([{ type: "GetUsage" }]);

    await emit(signedIn);

    expectPendingRows(screen.getByRole("region", { name: "Plan usage" }));
  });

  it("shows nothing without a signed-in session", async () => {
    const { emit } = await mountMeter(null);
    expect(screen.queryByRole("region", { name: "Plan usage" })).toBeNull();

    await emit({
      type: "SessionChanged",
      session: { type: "SignedOut", reason: null },
    });

    expect(screen.queryByRole("region", { name: "Plan usage" })).toBeNull();
    expect(screen.queryAllByRole("img", { hidden: true })).toHaveLength(0);
  });
});

describe("the usage meter, connected", () => {
  it("shows the 5-hour and weekly windows with the share used, the time until reset and the pace", async () => {
    const { meter } = await renderMeter(connected([fiveHours, weekly]));

    expect(within(meter).getAllByRole("img")).toHaveLength(2);
    expect(
      within(meter).getByRole("img", {
        name: "5-hour window, 38% used, 55% of the window elapsed",
      }),
    ).toBeInTheDocument();
    expect(
      within(meter).getByRole("img", {
        name: "Weekly window, 61% used, 47% of the window elapsed",
      }),
    ).toBeInTheDocument();
    expect(within(meter).getByText("5-hour")).toBeVisible();
    expect(within(meter).getByText("38% · resets in 2h 15m")).toBeVisible();
    expect(within(meter).getByText("Weekly")).toBeVisible();
    expect(within(meter).getByText("61% · resets in 3d 17h")).toBeVisible();
  });

  it("updates the figures as time passes, without a new reading", async () => {
    const { meter } = await renderMeter(connected([fiveHours, weekly]));

    await elapse(MINUTE);
    expect(within(meter).getByText("38% · resets in 2h 14m")).toBeVisible();

    await elapse(14 * MINUTE);
    expect(
      within(meter).getByRole("img", {
        name: "5-hour window, 38% used, 60% of the window elapsed",
      }),
    ).toBeInTheDocument();
    expect(within(meter).getByText("38% · resets in 2h")).toBeVisible();
  });

  it("shows one bar for a plan with only a weekly window", async () => {
    const { meter } = await renderMeter(connected([weekly]));

    expect(within(meter).getAllByRole("img")).toHaveLength(1);
    expect(
      within(meter).getByRole("img", {
        name: "Weekly window, 61% used, 47% of the window elapsed",
      }),
    ).toBeInTheDocument();
    expect(within(meter).queryByText("5-hour")).toBeNull();
  });

  it("shows a window that has not started without a pace mark or a countdown", async () => {
    const { meter } = await renderMeter(
      connected([win("5h", 5 * HOUR, 0, null), weekly]),
    );

    expect(
      within(meter).getByRole("img", {
        name: "5-hour window, 0% used, not started",
      }),
    ).toBeInTheDocument();
    expect(within(meter).getByText("0% · not started")).toBeVisible();
  });

  it("says nothing about the level of a window below 75 %", async () => {
    const { meter } = await renderMeter(connected([fiveHours, weekly]));

    for (const words of ["High", "Critical", "At limit"]) {
      expect(within(meter).queryByText(words)).toBeNull();
    }
  });

  it.each<[UsageLevel, number, string, string]>([
    ["High", 0.8, "High", "high"],
    ["Critical", 0.92, "Critical", "critical"],
    ["Limit", 1, "At limit", "at its limit"],
  ])(
    "names the %s level in words and in the text alternative",
    async (level, used, words, spoken) => {
      const { meter } = await renderMeter(
        connected([
          win("5h", 5 * HOUR, used, 2 * HOUR + 15 * MINUTE, level),
          weekly,
        ]),
      );

      expect(within(meter).getByText(words)).toBeVisible();
      expect(
        within(meter).getByRole("img", {
          name: `5-hour window, ${Math.round(used * 100)}% used, ${spoken}, 55% of the window elapsed`,
        }),
      ).toBeInTheDocument();
    },
  );

  it("lists the other windows the account reports in a popover", async () => {
    const { meter, user } = await renderMeter(
      connected([fiveHours, weekly, monthly]),
    );
    expect(within(meter).getAllByRole("img")).toHaveLength(2);

    await user.click(
      within(meter).getByRole("button", { name: "1 more window" }),
    );

    const popover = screen.getByRole("dialog", { name: "Other windows" });
    expect(within(popover).getByText("30-day")).toBeVisible();
    expect(within(popover).getByText("12% · resets in 17d")).toBeVisible();
    expect(
      within(popover).getByRole("img", {
        name: "30-day window, 12% used, 43% of the window elapsed",
      }),
    ).toBeInTheDocument();
  });

  it("offers no popover when the account reports only the two windows", async () => {
    const { meter } = await renderMeter(connected([fiveHours, weekly]));

    expect(within(meter).queryAllByRole("button")).toHaveLength(0);
  });
});

describe("a stale reading", () => {
  it.each<[UsageFailure, string]>([
    ["Network", "the network could not be reached"],
    ["Auth", "OpenAI did not accept the plan login"],
    ["Rejected", "OpenAI refused the request"],
    ["Timeout", "OpenAI did not answer in time"],
    ["Cancelled", "the reading was cancelled"],
    ["Unexpected", "OpenAI answered in a way Atlas does not understand"],
  ])(
    "keeps the last reading, marked, with the %s reason on focus",
    async (reason, sentence) => {
      const { meter } = await renderMeter(
        connected([fiveHours, weekly], reason),
      );

      expect(
        within(meter).getByRole("img", {
          name: "5-hour window, 38% used, 55% of the window elapsed",
        }),
      ).toBeInTheDocument();
      const mark = within(meter).getByRole("button", { name: /Out of date/ });
      expect(mark).toBeVisible();

      act(() => mark.focus());

      expect(await screen.findByRole("tooltip")).toHaveTextContent(
        `The latest reading failed: ${sentence}.`,
      );
    },
  );
});

describe("a usage meter that cannot read", () => {
  it("says usage could not be read when the first reading failed, and Try again asks for usage again", async () => {
    const { meter, requests, user } = await renderMeter({
      type: "FirstReadFailed",
      reason: "Timeout",
    });

    expect(meter).toHaveTextContent(
      "Usage could not be read: OpenAI did not answer in time.",
    );
    expect(requests).toEqual([{ type: "GetUsage" }]);
    await user.click(within(meter).getByRole("button", { name: "Try again" }));

    expect(requests).toEqual([{ type: "GetUsage" }, { type: "GetUsage" }]);
  });

  it("says the plan login was not accepted when that is why", async () => {
    const { meter } = await renderMeter({
      type: "FirstReadFailed",
      reason: "Auth",
    });

    expect(meter).toHaveTextContent(
      "Usage could not be read: OpenAI did not accept the plan login.",
    );
  });

  it("gives way to a reading that arrives afterwards", async () => {
    const { meter, emit } = await renderMeter({
      type: "FirstReadFailed",
      reason: "Network",
    });

    await emit({ type: "UsageChanged", usage: connected([fiveHours, weekly]) });

    expect(meter).not.toHaveTextContent("Usage could not be read");
    expect(
      within(meter).queryByRole("button", { name: "Try again" }),
    ).toBeNull();
    expect(
      within(meter).getByRole("img", {
        name: "5-hour window, 38% used, 55% of the window elapsed",
      }),
    ).toBeInTheDocument();
  });
});

describe("the folded meter in a narrow window", () => {
  const original = window.matchMedia;

  beforeEach(() => {
    // Below 1024 px: every width query the meter asks about holds.
    window.matchMedia = ((query: string) => ({
      matches: /max-width|width </.test(query),
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    })) as unknown as typeof window.matchMedia;
  });

  afterEach(() => {
    window.matchMedia = original;
  });

  it("shows the two bars only, with the figures on focus", async () => {
    const { meter } = await renderMeter(
      connected([fiveHours, weekly, monthly]),
      true,
    );

    expect(within(meter).getAllByRole("img")).toHaveLength(2);
    expect(within(meter).queryByText("38% · resets in 2h 15m")).toBeNull();
    expect(within(meter).queryAllByRole("button")).toHaveLength(0);

    const bar = within(meter).getByRole("img", {
      name: "5-hour window, 38% used, 55% of the window elapsed",
    });
    act(() => bar.focus());

    const tooltip = await screen.findByRole("tooltip");
    expect(tooltip).toHaveTextContent("5-hour");
    expect(tooltip).toHaveTextContent("38% · resets in 2h 15m");
  });

  it.each(states)("offers no Connect usage in %s", async (_tag, usage) => {
    await expectNoConnectUsage(usage, true);
  });

  it("shows nothing without a plan login", async () => {
    await mountMeter({ type: "NoPlanLogin" }, true);

    expect(screen.queryByRole("region", { name: "Plan usage" })).toBeNull();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });

  it("shows the two empty bars only while awaiting the first reading", async () => {
    const { meter } = await renderMeter({ type: "AwaitingReading" }, true);

    expect(within(meter).queryAllByRole("term", { hidden: true })).toHaveLength(
      0,
    );
    expect(
      within(meter).queryAllByRole("definition", { hidden: true }),
    ).toHaveLength(0);
    expectEmptyBars(meter);
  });

  it("offers Try again as an icon, the sentence as its tooltip, after a failed first reading", async () => {
    const { meter } = await renderMeter(
      { type: "FirstReadFailed", reason: "Network" },
      true,
    );

    expect(meter).not.toHaveTextContent("Usage could not be read");
    const button = within(meter).getByRole("button", { name: "Try again" });
    expect(button).not.toHaveTextContent("Try again");

    act(() => button.focus());

    expect(await screen.findByRole("tooltip")).toHaveTextContent(
      "Usage could not be read: the network could not be reached.",
    );
  });

  it("Try again after a failed first reading asks for usage again", async () => {
    const { meter, requests, user } = await renderMeter(
      { type: "FirstReadFailed", reason: "Network" },
      true,
    );

    await user.click(within(meter).getByRole("button", { name: "Try again" }));

    expect(requests).toEqual([{ type: "GetUsage" }, { type: "GetUsage" }]);
  });
});
