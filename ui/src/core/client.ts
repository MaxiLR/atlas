import type { CoreEvent } from "./generated/CoreEvent";
import type { CoreRequest } from "./generated/CoreRequest";
import type { CoreResponse } from "./generated/CoreResponse";

/**
 * The interface's only way to reach the core: one typed request in, one typed
 * response out, events on the side. A failed request rejects with a
 * `CoreError`.
 */
export interface CoreClient {
  call(request: CoreRequest): Promise<CoreResponse>;
  /** Calls `listener` for every core event; returns the unsubscribe. */
  subscribe(listener: (event: CoreEvent) => void): () => void;
}
