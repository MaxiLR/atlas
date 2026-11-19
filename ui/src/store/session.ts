import type { CoreError } from "@/core/generated/CoreError";
import type { Session } from "@/core/generated/Session";

export interface SessionSlice {
  /** Null until the core has said. */
  session: Session | null;
  /** Why the core could not say, until it is asked again. */
  error: CoreError | null;
}

export const initialSession: SessionSlice = { session: null, error: null };
