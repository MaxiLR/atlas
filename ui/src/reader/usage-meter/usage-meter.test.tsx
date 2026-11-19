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
 * The meter over a core that reports `usage`. `requests` records what the
 * meter asked; `emit` sends the meter an event from the core.
 */
async function renderMeter(usage: UsageState, folded = false) {
  const requests: CoreRequest[] = [];
  const listeners = new Set<(event: CoreEvent) => void>();
  const client: CoreClient = {
    call(request: CoreRequest) {
      requests.push(request);
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
  const meter = await screen.findByRole("region", { name: "Plan usage" });
  const emit = (event: CoreEvent) =>
    act(() => {
      for (const listener of listeners) listener(event);
    });
  return { meter, requests, emit, user: userEvent.setup() };
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

describe("the usage meter, not connected", () => {
  it("is empty but for Connect usage, which starts the usage login", async () => {
    const { meter, requests, user } = await renderMeter({
      type: "NotConnected",
    });

    expect(within(meter).getAllByRole("button")).toHaveLength(1);
    expect(within(meter).queryAllByRole("img")).toHaveLength(0);
    await user.click(
      within(meter).getByRole("button", { name: "Connect usage" }),
    );

    expect(requests.map(({ type }) => type)).toContain("ConnectUsage");
  });
});

describe("connecting usage", () => {
  const connecting: UsageState = {
    type: "Connecting",
    userCode: "QWRT-7KPD",
    verificationUrl: "https://auth.openai.com/codex/device",
    expiresAt: new Date(NOW + 15 * MINUTE).toISOString(),
  };

  it("shows the device code in a dialog with where to enter it and the time left", async () => {
    await renderMeter(connecting);

    const dialog = screen.getByRole("dialog", { name: "Connect usage" });
    expect(within(dialog).getByText("QWRT-7KPD")).toBeVisible();
    expect(dialog).toHaveTextContent(
      "OpenAI's page has opened in your browser",
    );
    expect(dialog).toHaveTextContent("Code expires in 15m");

    await elapse(MINUTE);
    expect(dialog).toHaveTextContent("Code expires in 14m");
  });

  it("copies the code", async () => {
    const { user } = await renderMeter(connecting);

    await user.click(screen.getByRole("button", { name: "Copy code" }));

    expect(await navigator.clipboard.readText()).toBe("QWRT-7KPD");
  });

  it("cancels the usage login from the dialog", async () => {
    const { requests, user } = await renderMeter(connecting);

    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Cancel",
      }),
    );

    expect(requests.map(({ type }) => type)).toContain("CancelConnectUsage");
  });

  it("closes once the usage login completes and shows the bars", async () => {
    const { emit } = await renderMeter(connecting);

    await emit({ type: "UsageChanged", usage: connected([fiveHours, weekly]) });

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(
      screen.getByRole("img", {
        name: "5-hour window, 38% used, 55% of the window elapsed",
      }),
    ).toBeInTheDocument();
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
    ["Auth", "OpenAI no longer accepts the usage login"],
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
  it("says usage could not be read when the first reading failed, and offers to try again", async () => {
    const { meter, requests, user } = await renderMeter({
      type: "FirstReadFailed",
      reason: "Timeout",
    });

    expect(meter).toHaveTextContent(
      "Usage could not be read: OpenAI did not answer in time.",
    );
    await user.click(within(meter).getByRole("button", { name: "Try again" }));

    expect(requests.map(({ type }) => type)).toContain("ConnectUsage");
  });

  it("offers Connect usage again once the usage login ended", async () => {
    const { meter, requests, user } = await renderMeter({ type: "SignedOut" });

    expect(within(meter).queryAllByRole("img")).toHaveLength(0);
    await user.click(
      within(meter).getByRole("button", { name: "Connect usage" }),
    );

    expect(requests.map(({ type }) => type)).toContain("ConnectUsage");
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
});
