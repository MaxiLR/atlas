import type { CourseView } from "@/core/generated/CourseView";
import type { PlacementState } from "@/core/generated/PlacementState";
import type { Session } from "@/core/generated/Session";

import { unknownTag } from "./unknown-tag";

/** The screen the window shows. */
export type Screen = "signIn" | "library" | "settings" | "placement" | "reader";

/** The side panel's tabs. */
export type PanelTab = "glossary" | "sideChats";

/** What covers the reading column below 1024 px. */
export type Overlay = "index" | "panel";

export interface NavigationSlice {
  screen: Screen;
  courseId: string | null;
  sectionId: string | null;
  /** The section Next was asked from, until the core says where it leads. */
  nextFrom: string | null;
  /** Next was asked from the last section of the course. */
  endOfCourse: boolean;
  panel: { open: boolean; tab: PanelTab; chatId: string | null };
  overlay: Overlay | null;
}

export const initialNavigation: NavigationSlice = {
  screen: "library",
  courseId: null,
  sectionId: null,
  nextFrom: null,
  endOfCourse: false,
  panel: { open: false, tab: "glossary", chatId: null },
  overlay: null,
};

/** What the interface does on its own: screens, the panel, overlays. */
export type NavigationAction =
  | { type: "ShowLibrary" }
  | { type: "ShowSettings" }
  | { type: "OpenPanel"; tab: PanelTab }
  | { type: "ClosePanel" }
  /** Leaves the open side chat for the course's list. */
  | { type: "ShowSideChatList" }
  | { type: "OpenOverlay"; overlay: Overlay }
  | { type: "CloseOverlay" };

/** The window after an action of the interface's own. */
export function navigate(
  nav: NavigationSlice,
  action: NavigationAction,
): NavigationSlice {
  switch (action.type) {
    case "ShowLibrary":
      return { ...initialNavigation, screen: "library" };
    case "ShowSettings":
      return { ...nav, screen: "settings", overlay: null };
    case "OpenPanel":
      return { ...nav, panel: { ...nav.panel, open: true, tab: action.tab } };
    case "ClosePanel":
      return { ...nav, panel: { ...nav.panel, open: false } };
    case "ShowSideChatList":
      return { ...nav, panel: { ...nav.panel, chatId: null } };
    case "OpenOverlay":
      return { ...nav, overlay: action.overlay };
    case "CloseOverlay":
      return { ...nav, overlay: null };
    default:
      return unknownTag(action);
  }
}

/**
 * Signed out leads to sign-in, whatever was open; signed in leads from
 * sign-in to the library.
 */
export function sessionNavigated(
  nav: NavigationSlice,
  session: Session,
): NavigationSlice {
  switch (session.type) {
    case "SignedOut":
      return { ...initialNavigation, screen: "signIn" };
    case "SigningIn":
      return nav;
    case "SignedIn":
      return nav.screen === "signIn" ? { ...nav, screen: "library" } : nav;
    default:
      return unknownTag(session);
  }
}

/** A course opened: placement while it is in placement, otherwise its reader. */
export function courseNavigated(
  nav: NavigationSlice,
  course: CourseView,
): NavigationSlice {
  return {
    ...nav,
    screen: course.stage === "Placement" ? "placement" : "reader",
    courseId: course.id,
    sectionId: null,
    nextFrom: null,
    endOfCourse: false,
    overlay: null,
  };
}

/** Placement of the open course is done: its index is next. */
export function placementNavigated(
  nav: NavigationSlice,
  courseId: string,
  placement: PlacementState,
): NavigationSlice {
  return placement.step.type === "Done" &&
    nav.screen === "placement" &&
    nav.courseId === courseId
    ? { ...nav, screen: "reader" }
    : nav;
}

/** A section of a course is shown. */
export function sectionNavigated(
  nav: NavigationSlice,
  courseId: string,
  sectionId: string,
): NavigationSlice {
  return {
    ...nav,
    screen: "reader",
    courseId,
    sectionId,
    nextFrom: null,
    endOfCourse: false,
  };
}
