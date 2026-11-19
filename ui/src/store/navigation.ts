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
  | { type: "OpenOverlay"; overlay: Overlay }
  | { type: "CloseOverlay" };
