import { describe, expect, it } from "vitest";

import type { Session } from "@/core/generated/Session";

import {
  applyFailure,
  applyNavigation,
  applyResponse,
  initialState,
  type AtlasState,
} from "./index";

const signedIn: Session = {
  type: "SignedIn",
  account: { email: "candidate@example.com" },
  planUsage: "On",
};

/** The core could not read the session: its secret store failed. */
function sessionUnread(
  message = "secrets.json is not valid JSON.",
): AtlasState {
  const state = applyFailure(
    initialState,
    { type: "GetSession" },
    { type: "Store", message },
  );
  expect(state.notices.map((notice) => notice.id)).toEqual(["session"]);
  return state;
}

describe("the session notice", () => {
  it("shows when the session could not be read, with the reason and Retry", () => {
    expect(sessionUnread().notices).toEqual([
      {
        id: "session",
        kind: "error",
        title: "Session could not be read",
        description: "secrets.json is not valid JSON.",
        action: { label: "Retry", request: { type: "GetSession" } },
      },
    ]);
  });

  it("closes when a later GetSession answers with the session", () => {
    const after = applyResponse(
      sessionUnread(),
      { type: "GetSession" },
      { type: "Session", session: signedIn },
    );

    expect(after.notices).toEqual([]);
  });

  it("stays when the session arrives as the answer to another request", () => {
    const after = applyResponse(
      sessionUnread(),
      { type: "CancelSignIn" },
      { type: "Session", session: { type: "SignedOut", reason: null } },
    );

    expect(after.notices.map((notice) => notice.id)).toEqual(["session"]);
  });

  it("goes when dismissed", () => {
    const after = applyNavigation(sessionUnread(), {
      type: "DismissNotice",
      id: "session",
    });

    expect(after.notices).toEqual([]);
  });

  it("dismissing another cause leaves it showing", () => {
    const after = applyNavigation(sessionUnread(), {
      type: "DismissNotice",
      id: "pdf",
    });

    expect(after.notices.map((notice) => notice.id)).toEqual(["session"]);
  });

  it("a second failure replaces the first instead of stacking", () => {
    const again = applyFailure(
      sessionUnread("secrets.json is not valid JSON."),
      { type: "GetSession" },
      { type: "Unavailable", feature: "session" },
    );

    expect(again.notices).toHaveLength(1);
    expect(again.notices[0]).toMatchObject({
      id: "session",
      description: "session is not available yet.",
    });
  });
});
