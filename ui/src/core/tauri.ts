import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { CoreClient } from "./client";
import type { CoreEvent } from "./generated/CoreEvent";
import type { CoreResponse } from "./generated/CoreResponse";

/** The Tauri command that passes a request to the core. */
export const CORE_COMMAND = "core_request";
/** The Tauri event that carries the core's events. */
export const CORE_EVENT = "core-event";

export function createTauriCore(): CoreClient {
  return {
    call: (request) => invoke<CoreResponse>(CORE_COMMAND, { request }),
    subscribe(listener) {
      const unlisten = listen<CoreEvent>(CORE_EVENT, (event) =>
        listener(event.payload),
      );
      return () => {
        void unlisten.then((stop) => stop());
      };
    },
  };
}
