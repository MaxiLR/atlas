import type { PlacementState } from "@/core/generated/PlacementState";

export interface PlacementSlice {
  /** Where placement stands, by course. */
  byCourse: Record<string, PlacementState>;
}

export const initialPlacement: PlacementSlice = { byCourse: {} };

export function placementChanged(
  slice: PlacementSlice,
  courseId: string,
  placement: PlacementState,
): PlacementSlice {
  return { byCourse: { ...slice.byCourse, [courseId]: placement } };
}
