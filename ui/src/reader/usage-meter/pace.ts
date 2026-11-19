// How far through a window the clock is, and how long until it resets.
// Ported from Headroom's `logic/pace.js`.

import type { UsageWindow } from "@/core/generated/UsageWindow";

const MINUTE = 60 * 1000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

type Timed = Pick<UsageWindow, "resetsAt" | "lengthSeconds">;

/**
 * The share of `window`'s length that has passed at `now` (ms), 0 to 1; null
 * for a window that has not started.
 */
export function paceOf(window: Timed, now: number): number | null {
  if (window.resetsAt === null) return null;
  const left = Date.parse(window.resetsAt) - now;
  const pace = 1 - left / (window.lengthSeconds * 1000);
  return Math.min(1, Math.max(0, pace));
}

/** `ms` in its two largest whole units: `<1m`, `42m`, `3h 12m`, `2d 4h`. */
export function countdown(ms: number): string {
  if (ms < MINUTE) return "<1m";
  if (ms < HOUR) return `${Math.floor(ms / MINUTE)}m`;
  if (ms < DAY) {
    const hours = Math.floor(ms / HOUR);
    const minutes = Math.floor((ms % HOUR) / MINUTE);
    return minutes ? `${hours}h ${minutes}m` : `${hours}h`;
  }
  const days = Math.floor(ms / DAY);
  const hours = Math.floor((ms % DAY) / HOUR);
  return hours ? `${days}d ${hours}h` : `${days}d`;
}

/** The time left until `window` resets; null for a window that has not started. */
export function untilReset(window: Timed, now: number): string | null {
  return window.resetsAt === null
    ? null
    : countdown(Date.parse(window.resetsAt) - now);
}
