import { describe, expect, it } from "vitest";

import type { UsageWindow } from "@/core/generated/UsageWindow";

import { countdown, paceOf, untilReset } from "./pace";

// The cases of Headroom's `tests/pace.test.mjs`, ported.

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** A fixed moment: 12 November 2026, 09:00 UTC. */
const NOW = Date.UTC(2026, 10, 12, 9, 0, 0);

const LENGTHS: Record<string, number> = { "5h": 5 * 3600, "7d": 7 * 86_400 };

/** A window named `name`, `used` of it spent, resetting at `resetsAt` (ms). */
function win(name: string, used: number, resetsAt: number | null): UsageWindow {
  return {
    name,
    lengthSeconds: LENGTHS[name]!,
    used,
    resetsAt: resetsAt === null ? null : new Date(resetsAt).toISOString(),
    level: "Normal",
  };
}

describe("paceOf and untilReset", () => {
  it("a weekly window resetting in 4d 6h has its mark at 66/168", () => {
    const weekly = win("7d", 0.41, NOW + 4 * DAY + 6 * HOUR);
    expect(paceOf(weekly, NOW)).toBeCloseTo(66 / 168, 9);
    expect(untilReset(weekly, NOW)).toBe("4d 6h");
  });

  it("a 5-hour window resetting in 1h 48m has its mark at 0.64", () => {
    const short = win("5h", 0.62, NOW + HOUR + 48 * MINUTE);
    expect(paceOf(short, NOW)).toBeCloseTo(0.64, 9);
    expect(untilReset(short, NOW)).toBe("1h 48m");
  });

  it("a reset time in the past gives a full mark and <1m", () => {
    const past = win("5h", 0.3, NOW - 10 * MINUTE);
    expect(paceOf(past, NOW)).toBe(1);
    expect(untilReset(past, NOW)).toBe("<1m");
  });

  it("a reset time further than the window's length gives an empty mark", () => {
    const ahead = win("5h", 0.3, NOW + 6 * HOUR);
    expect(paceOf(ahead, NOW)).toBe(0);
  });

  it("a window that has not started has no mark and no countdown", () => {
    const idle = win("5h", 0, null);
    expect(paceOf(idle, NOW)).toBeNull();
    expect(untilReset(idle, NOW)).toBeNull();
  });
});

describe("countdown", () => {
  it("below one minute is <1m, zero and negative too", () => {
    expect(countdown(59 * SECOND)).toBe("<1m");
    expect(countdown(0)).toBe("<1m");
    expect(countdown(-5 * MINUTE)).toBe("<1m");
  });

  it("below one hour gives whole minutes", () => {
    expect(countdown(MINUTE)).toBe("1m");
    expect(countdown(MINUTE + 59 * SECOND)).toBe("1m");
    expect(countdown(42 * MINUTE + 30 * SECOND)).toBe("42m");
    expect(countdown(59 * MINUTE + 59 * SECOND)).toBe("59m");
  });

  it("below one day gives hours and minutes", () => {
    expect(countdown(3 * HOUR + 12 * MINUTE)).toBe("3h 12m");
    expect(countdown(3 * HOUR + 12 * MINUTE + 59 * SECOND)).toBe("3h 12m");
    expect(countdown(3 * HOUR)).toBe("3h");
    expect(countdown(3 * HOUR + 40 * SECOND)).toBe("3h");
    expect(countdown(21 * HOUR)).toBe("21h");
    expect(countdown(23 * HOUR + 59 * MINUTE)).toBe("23h 59m");
  });

  it("from one day gives days and hours", () => {
    expect(countdown(2 * DAY + 4 * HOUR)).toBe("2d 4h");
    expect(countdown(2 * DAY + 4 * HOUR + 59 * MINUTE)).toBe("2d 4h");
    expect(countdown(5 * DAY + 2 * HOUR)).toBe("5d 2h");
    expect(countdown(DAY)).toBe("1d");
    expect(countdown(2 * DAY + 30 * MINUTE)).toBe("2d");
  });
});
