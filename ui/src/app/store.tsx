import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { createStore, useStore, type StoreApi } from "zustand";
import { useShallow } from "zustand/react/shallow";

import { describeError, type CoreClient, type ResponseTo } from "@/core/client";
import { useCore } from "@/core/context";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CoreResponse } from "@/core/generated/CoreResponse";
import {
  applyEvent,
  applyFailure,
  applyNavigation,
  applyRequest,
  applyResponse,
  initialState,
  type AtlasState,
  type NavigationAction,
} from "@/store";

/** The store, fed only by the core's events and the answers to requests. */
export interface AtlasStore {
  state: StoreApi<AtlasState>;
  /** Starts taking in the core's events; returns the stop. */
  connect(): () => void;
  /**
   * Sends a request through the store: the store takes in the request, then
   * the response or the failure. Resolves and rejects as the core does. When
   * Next is answered with where it leads and the window followed, it opens
   * that section too.
   */
  send<Request extends CoreRequest>(
    request: Request,
  ): Promise<ResponseTo<Request>>;
  /** An action of the interface's own: screens, the panel, overlays. */
  navigate(action: NavigationAction): void;
  /**
   * Whether the window has moved to another screen since it first showed
   * one. A screen reached that way takes focus; the first one does not.
   */
  screenChanged(): boolean;
}

/**
 * The core's error, or for a failure outside the core (the transport, or an
 * answer the request does not promise) an invalid request with its sentence.
 */
function asCoreError(error: unknown): CoreError {
  return typeof error === "object" && error !== null && "type" in error
    ? (error as CoreError)
    : { type: "InvalidRequest", message: describeError(error) };
}

export function createAtlasStore(client: CoreClient): AtlasStore {
  const state = createStore<AtlasState>()(() => initialState);
  const apply = (change: (current: AtlasState) => AtlasState) =>
    state.setState(change, true);
  const send: AtlasStore["send"] = async (request) => {
    apply((current) => applyRequest(current, request));
    try {
      const response = await client.call(request);
      apply((current) => applyResponse(current, request, response));
      const answer: CoreResponse = response;
      if (
        answer.type === "NextSection" &&
        state.getState().navigation.sectionId === answer.sectionId
      ) {
        send({
          type: "OpenSection",
          courseId: answer.courseId,
          sectionId: answer.sectionId,
        }).catch(failureInStore);
      }
      return response;
    } catch (error) {
      apply((current) => applyFailure(current, request, asCoreError(error)));
      throw error;
    }
  };
  // The first screen is the one shown once the session is known.
  let firstScreen: AtlasState["navigation"]["screen"] | null = null;
  let screenChanged = false;
  state.subscribe((current) => {
    if (screenChanged || current.session.session === null) return;
    firstScreen ??= current.navigation.screen;
    screenChanged = current.navigation.screen !== firstScreen;
  });
  return {
    state,
    connect: () =>
      client.subscribe((event) =>
        apply((current) => applyEvent(current, event)),
      ),
    send,
    navigate: (action) => apply((current) => applyNavigation(current, action)),
    screenChanged: () => screenChanged,
  };
}

/**
 * Handles the rejection of a request whose failure the store records, so a
 * screen, or a notice when the screen has no place for it, shows it from the
 * store.
 */
export function failureInStore(): void {}

const StoreContext = createContext<AtlasStore | null>(null);

/** Builds the store over the core from `CoreProvider` and keeps it connected. */
export function AtlasStoreProvider({ children }: { children: ReactNode }) {
  const core = useCore();
  const [store] = useState(() => createAtlasStore(core));
  useEffect(() => store.connect(), [store]);
  return <StoreContext value={store}>{children}</StoreContext>;
}

export function useAtlasStore(): AtlasStore {
  const store = useContext(StoreContext);
  if (store === null) {
    throw new Error("useAtlasStore needs an AtlasStoreProvider");
  }
  return store;
}

/**
 * Reads the store. The selector returns values held in the state, or an
 * object of them, compared field by field; one that builds new arrays needs
 * `useMemo` over the values it reads instead.
 */
export function useAtlas<T>(selector: (state: AtlasState) => T): T {
  return useStore(useAtlasStore().state, useShallow(selector));
}
