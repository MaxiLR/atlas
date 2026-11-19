import { describe, expect, it } from "vitest";

import type { CourseSummary } from "@/core/generated/CourseSummary";
import type { CourseView } from "@/core/generated/CourseView";
import type { Session } from "@/core/generated/Session";

import {
  applyEvent,
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
});
