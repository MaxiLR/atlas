import type { Session } from "@/core/generated/Session";

export interface SessionSlice {
  /** Null until the core has said. */
  session: Session | null;
}

export const initialSession: SessionSlice = { session: null };
