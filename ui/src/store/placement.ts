import type { PlacementState } from "@/core/generated/PlacementState";

export interface PlacementSlice {
  /** Where placement stands, by course. */
  byCourse: Record<string, PlacementState>;
}

export const initialPlacement: PlacementSlice = { byCourse: {} };
