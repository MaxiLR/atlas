import type { UsageState } from "@/core/generated/UsageState";

export interface UsageSlice {
  /** Null until the core has said. */
  state: UsageState | null;
}

export const initialUsage: UsageSlice = { state: null };
