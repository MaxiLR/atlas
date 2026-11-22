import { describe, expect, it } from "vitest";

import type { UsageState } from "@/core/generated/UsageState";
import type { UsageWindow } from "@/core/generated/UsageWindow";

import { applyEvent, applyResponse, initialState, selectUsage } from "./index";

const windows: UsageWindow[] = [
  {
    name: "5h",
    lengthSeconds: 18_000,
    used: 0.42,
    resetsAt: "2026-11-19T14:00:00Z",
    level: "Normal",
  },
  {
    name: "7d",
    lengthSeconds: 604_800,
    used: 0.8,
    resetsAt: "2026-11-23T00:00:00Z",
    level: "High",
  },
];

const connected = applyEvent(initialState, {
  type: "UsageChanged",
  usage: {
    type: "Connected",
    reading: { windows, takenAt: "2026-11-19T10:00:00Z", stale: null },
  },
});

describe("a failed usage reading", () => {
  it("keeps the last windows, marked stale with the reason", () => {
    const after = applyEvent(connected, {
      type: "UsageChanged",
      usage: {
        type: "Connected",
        reading: {
          windows: [],
          takenAt: "2026-11-19T10:05:00Z",
          stale: "Network",
        },
      },
    });

    expect(selectUsage(after)).toEqual<UsageState>({
      type: "Connected",
      reading: { windows, takenAt: "2026-11-19T10:00:00Z", stale: "Network" },
    });
  });

  it("shows the reason when the core reports the last reading as stale", () => {
    const after = applyResponse(
      connected,
      { type: "GetUsage" },
      {
        type: "Usage",
        usage: {
          type: "Connected",
          reading: {
            windows,
            takenAt: "2026-11-19T10:00:00Z",
            stale: "Timeout",
          },
        },
      },
    );

    expect(selectUsage(after)).toEqual<UsageState>({
      type: "Connected",
      reading: { windows, takenAt: "2026-11-19T10:00:00Z", stale: "Timeout" },
    });
  });

  it("keeps the last windows when a reading fails as if it were the first", () => {
    const after = applyEvent(connected, {
      type: "UsageChanged",
      usage: { type: "FirstReadFailed", reason: "Auth" },
    });

    expect(selectUsage(after)).toEqual<UsageState>({
      type: "Connected",
      reading: { windows, takenAt: "2026-11-19T10:00:00Z", stale: "Auth" },
    });
  });

  it("drops the windows once the plan login ends", () => {
    const after = applyEvent(connected, {
      type: "UsageChanged",
      usage: { type: "NoPlanLogin" },
    });

    expect(selectUsage(after)).toEqual<UsageState>({ type: "NoPlanLogin" });
  });

  it("has no windows to keep for the next plan login's failed first reading", () => {
    const signedOut = applyEvent(connected, {
      type: "UsageChanged",
      usage: { type: "NoPlanLogin" },
    });
    const awaiting = applyEvent(signedOut, {
      type: "UsageChanged",
      usage: { type: "AwaitingReading" },
    });
    expect(selectUsage(awaiting)).toEqual<UsageState>({
      type: "AwaitingReading",
    });

    const after = applyEvent(awaiting, {
      type: "UsageChanged",
      usage: { type: "FirstReadFailed", reason: "Auth" },
    });

    expect(selectUsage(after)).toEqual<UsageState>({
      type: "FirstReadFailed",
      reason: "Auth",
    });
  });
});
