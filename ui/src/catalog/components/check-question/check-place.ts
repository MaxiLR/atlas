import { createContext, useContext } from "react";

import type { CheckResult } from "@/core/generated/CheckResult";

/** Where a check question shown in a section sits, and how it was answered. */
export interface CheckPlace {
  /** The stored version it belongs to; null while the section is written. */
  versionId: string | null;
  /** The index of its check block in the version. */
  block: number;
  /** Its index in the block. */
  question: number;
  /** The core's result once answered; null before. */
  answer: CheckResult | null;
}

/**
 * Set by the section's block list around each check question it mounts
 * through the registry, so the question can be answered.
 */
export const CheckPlaceContext = createContext<CheckPlace | null>(null);

/** The place of the check question being rendered; null outside a section. */
export function useCheckPlace(): CheckPlace | null {
  return useContext(CheckPlaceContext);
}
