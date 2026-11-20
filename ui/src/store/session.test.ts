import { describe, expect, it } from "vitest";

import type { CourseSummary } from "@/core/generated/CourseSummary";
import type { CourseView } from "@/core/generated/CourseView";
import type { Session } from "@/core/generated/Session";

import {
  applyEvent,
  applyFailure,
  applyRequest,
  applyResponse,
  initialState,
  type AtlasState,
} from "./index";

const at = "2026-11-19T10:00:00Z";

const courses: CourseSummary[] = [
  {
    id: "course-1",
    topic: "Plate tectonics",
    stage: "Ready",
    progress: { read: 1, total: 3 },
    resumeTitle: "Plate boundaries",
    lastOpenedAt: at,
    createdAt: at,
    updatedAt: at,
  },
];

const course: CourseView = {
  id: "course-1",
  topic: "Plate tectonics",
  stage: "Ready",
  chapters: [],
  resumeAt: null,
  glossaryCount: 0,
  sideChatCount: 0,
  placement: null,
};

const signedIn: Session = {
  type: "SignedIn",
  account: { email: "candidate@example.com" },
  planUsage: "On",
};

/** Signed in, reading a section of a course from the library. */
function reading(): AtlasState {
  let state = applyEvent(initialState, {
    type: "SessionChanged",
    session: signedIn,
  });
  state = applyResponse(
    state,
    { type: "ListCourses" },
    {
      type: "Courses",
      courses,
    },
  );
  state = applyResponse(
    state,
    { type: "OpenCourse", courseId: "course-1" },
    { type: "Course", course },
  );
  return applyRequest(state, {
    type: "OpenSection",
    courseId: "course-1",
    sectionId: "section-1",
  });
}

describe("the session", () => {
  it("moves navigation to sign-in when it ends, keeping the library", () => {
    const before = reading();
    expect(before.navigation.screen).toBe("reader");

    const after = applyEvent(before, {
      type: "SessionChanged",
      session: { type: "SignedOut", reason: { type: "LoginEnded" } },
    });

    expect(after.navigation.screen).toBe("signIn");
    expect(after.session.session).toEqual({
      type: "SignedOut",
      reason: { type: "LoginEnded" },
    });
    expect(after.library.courses).toEqual(courses);
  });

  it("moves to sign-in when signing out answers signed out", () => {
    const after = applyResponse(
      reading(),
      { type: "SignOut" },
      {
        type: "Session",
        session: { type: "SignedOut", reason: { type: "SignedOut" } },
      },
    );

    expect(after.navigation.screen).toBe("signIn");
    expect(after.library.courses).toEqual(courses);
  });

  it("returns to the library once signed in again", () => {
    const signedOut = applyEvent(reading(), {
      type: "SessionChanged",
      session: { type: "SignedOut", reason: null },
    });
    const signingIn = applyEvent(signedOut, {
      type: "SessionChanged",
      session: { type: "SigningIn" },
    });
    expect(signingIn.navigation.screen).toBe("signIn");

    const back = applyEvent(signingIn, {
      type: "SessionChanged",
      session: signedIn,
    });
    expect(back.navigation.screen).toBe("library");
    expect(back.library.courses).toEqual(courses);
  });

  it("stays on sign-in when the sign-in returns without plan usage on", () => {
    for (const planUsage of ["Off", "NotAllowed"] as const) {
      const signingIn = applyEvent(
        applyEvent(initialState, {
          type: "SessionChanged",
          session: { type: "SignedOut", reason: null },
        }),
        { type: "SessionChanged", session: { type: "SigningIn" } },
      );

      const back = applyEvent(signingIn, {
        type: "SessionChanged",
        session: { ...signedIn, planUsage },
      });

      expect(back.navigation.screen).toBe("signIn");
      expect(back.session.session).toEqual({ ...signedIn, planUsage });
    }
  });

  it("keeps a finished sign-in when the reply that started it arrives late", () => {
    const signedOut = applyEvent(initialState, {
      type: "SessionChanged",
      session: { type: "SignedOut", reason: null },
    });
    const asked = applyRequest(signedOut, { type: "SignIn" });
    const back = applyEvent(asked, {
      type: "SessionChanged",
      session: { ...signedIn, planUsage: "Off" },
    });

    const late = applyResponse(
      back,
      { type: "SignIn" },
      { type: "Session", session: { type: "SigningIn" } },
    );

    expect(late.session.session).toEqual({ ...signedIn, planUsage: "Off" });
    expect(late.navigation.screen).toBe("signIn");
  });
});

describe("asking for the session", () => {
  const unavailable = { type: "Unavailable", feature: "session" } as const;

  it("keeps the core's refusal while the session is unknown", () => {
    const asked = applyRequest(initialState, { type: "GetSession" });
    const refused = applyFailure(asked, { type: "GetSession" }, unavailable);

    expect(refused.session).toEqual({ session: null, error: unavailable });
    expect(refused.navigation.screen).toBe("library");
  });

  it("forgets the refusal when asked again and when the session arrives", () => {
    const refused = applyFailure(
      initialState,
      { type: "GetSession" },
      unavailable,
    );

    expect(applyRequest(refused, { type: "GetSession" }).session.error).toBe(
      null,
    );
    expect(
      applyResponse(
        refused,
        { type: "GetSession" },
        { type: "Session", session: signedIn },
      ).session,
    ).toEqual({ session: signedIn, error: null });
  });
});
