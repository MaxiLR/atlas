import type { UsageWindow } from "@/core/generated/UsageWindow";

type Timed = Pick<UsageWindow, "resetsAt" | "lengthSeconds">;

export function paceOf(_window: Timed, _now: number): number | null {
  throw new Error("not built yet");
}

export function countdown(_ms: number): string {
  throw new Error("not built yet");
}

export function untilReset(_window: Timed, _now: number): string | null {
  throw new Error("not built yet");
}
