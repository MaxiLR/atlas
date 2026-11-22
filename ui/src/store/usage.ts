import type { UsageState } from "@/core/generated/UsageState";

export interface UsageSlice {
  /** Null until the core has said. */
  state: UsageState | null;
}

export const initialUsage: UsageSlice = { state: null };

/**
 * The meter after the core reported `next`. A failed reading that arrives
 * without windows keeps the last reading's windows and time, stale with the
 * reason; `NoPlanLogin`, once the plan login ends, drops them.
 */
export function usageChanged(slice: UsageSlice, next: UsageState): UsageSlice {
  const last = slice.state?.type === "Connected" ? slice.state.reading : null;
  const failure =
    next.type === "FirstReadFailed"
      ? next.reason
      : next.type === "Connected" && next.reading.windows.length === 0
        ? next.reading.stale
        : null;
  return last !== null && failure !== null
    ? { state: { type: "Connected", reading: { ...last, stale: failure } } }
    : { state: next };
}
