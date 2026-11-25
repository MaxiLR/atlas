import { describeError } from "@/core/client";
import type { CoreError } from "@/core/generated/CoreError";
import type { Session } from "@/core/generated/Session";

import type { Notice } from "./notices";

export interface SessionSlice {
  /** Null until the core has said. */
  session: Session | null;
  /** Why the core could not say, until it is asked again. */
  error: CoreError | null;
}

export const initialSession: SessionSlice = { session: null, error: null };

/** The cause of the notice saying the session could not be read. */
export const SESSION_NOTICE = "session";

/** The core could not read the session: why, and Retry to ask again. */
export function sessionNotice(error: CoreError): Notice {
  return {
    id: SESSION_NOTICE,
    kind: "error",
    title: "Session could not be read",
    description: describeError(error),
    action: { label: "Retry", request: { type: "GetSession" } },
  };
}
