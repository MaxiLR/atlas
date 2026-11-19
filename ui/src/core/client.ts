import type { CoreError } from "./generated/CoreError";
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

/** The untyped channel a client is built over: Tauri's command, or the fake core. */
export interface CoreTransport {
  request(request: CoreRequest): Promise<CoreResponse>;
  subscribe(listener: (event: CoreEvent) => void): () => void;
}

export function createCoreClient(transport: CoreTransport): CoreClient {
  return {
    call: (request) => transport.request(request),
    subscribe: (listener) => transport.subscribe(listener),
  };
}

/** Narrows a response to the variant a request promises; anything else is a bug. */
export function expectResponse<T extends CoreResponse["type"]>(
  response: CoreResponse,
  type: T,
): Extract<CoreResponse, { type: T }> {
  if (response.type !== type) {
    throw new Error(`expected a ${type} response, got ${response.type}`);
  }
  return response as Extract<CoreResponse, { type: T }>;
}

/** A sentence for a failed request, whether the core or the transport failed. */
export function describeError(error: unknown): string {
  if (typeof error === "object" && error !== null && "type" in error) {
    const coreError = error as CoreError;
    switch (coreError.type) {
      case "NotFound":
        return `Not found: ${coreError.what}.`;
      case "Unavailable":
        return `${coreError.feature} is not available yet.`;
      case "InvalidRequest":
      case "Provider":
      case "Store":
        return coreError.message;
      case "PlanLimit":
        return "The plan's usage limit is reached.";
      case "PlanUsageOff":
        return "Plan usage is off.";
      case "PlanNotAllowed":
        return "Plan usage is not allowed for this account.";
      case "SignInRequired":
        return "Sign in with ChatGPT first.";
      case "Offline":
        return "The app is offline.";
      case "FailedAfterRetries":
        return `The request failed after ${coreError.attempts} attempts.`;
    }
  }
  return error instanceof Error ? error.message : String(error);
}
