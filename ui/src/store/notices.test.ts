import { describe, expect, it } from "vitest";

import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { ExportResult } from "@/core/generated/ExportResult";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";
import type { Session } from "@/core/generated/Session";
import type { SideChatSummary } from "@/core/generated/SideChatSummary";

import {
  applyFailure,
  applyNavigation,
  applyResponse,
  initialState,
  type AtlasState,
  type Screen,
} from "./index";
import { initialNavigation } from "./navigation";

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

/** The state while the window shows `screen`. */
function on(screen: Screen): AtlasState {
  return { ...initialState, navigation: { ...initialNavigation, screen } };
}

const exportPdf: CoreRequest = { type: "ExportPdf", courseId: "course-1" };

/** The core answered the export with `result`. */
function exported(state: AtlasState, result: ExportResult): AtlasState {
  return applyResponse(state, exportPdf, { type: "Exported", result });
}

describe("the PDF export notices", () => {
  it("a saved PDF shows a success notice naming the file", () => {
    const after = exported(on("reader"), {
      type: "Saved",
      path: "/Users/candidate/Desktop/Plates, revised.pdf",
    });

    expect(after.notices).toEqual([
      {
        id: expect.any(String),
        kind: "success",
        title: "PDF saved",
        description: "Plates, revised.pdf",
        action: null,
      },
    ]);
  });

  it("a failed export shows an error notice with the core's reason", () => {
    const after = exported(on("reader"), {
      type: "Failed",
      reason: "Not enough free space to write Plate tectonics.pdf.",
    });

    expect(after.notices).toEqual([
      {
        id: expect.any(String),
        kind: "error",
        title: "PDF export failed",
        description: "Not enough free space to write Plate tectonics.pdf.",
        action: null,
      },
    ]);
  });

  it("an export the core refused shows an error notice with the refusal", () => {
    const after = applyFailure(on("reader"), exportPdf, { type: "Offline" });

    expect(after.notices).toEqual([
      {
        id: expect.any(String),
        kind: "error",
        title: "PDF export failed",
        description: "The app is offline.",
        action: null,
      },
    ]);
  });

  it("a cancelled export shows nothing", () => {
    expect(exported(on("reader"), { type: "Cancelled" }).notices).toEqual([]);
  });

  it("a saved PDF after a failed export replaces the failure", () => {
    const failed = exported(on("reader"), {
      type: "Failed",
      reason: "The disk is full.",
    });
    const saved = exported(failed, {
      type: "Saved",
      path: "/tmp/Plate tectonics.pdf",
    });

    expect(saved.notices).toHaveLength(1);
    expect(saved.notices[0]).toMatchObject({
      kind: "success",
      title: "PDF saved",
      description: "Plate tectonics.pdf",
    });
  });

  it("a failure after a refusal replaces it instead of stacking", () => {
    const refused = applyFailure(on("reader"), exportPdf, { type: "Offline" });
    const failed = exported(refused, {
      type: "Failed",
      reason: "The disk is full.",
    });

    expect(failed.notices).toHaveLength(1);
    expect(failed.notices[0]).toMatchObject({
      description: "The disk is full.",
    });
  });

  it("a failure on the library, which says it in its own line, shows no notice", () => {
    const refused = applyFailure(on("library"), exportPdf, { type: "Offline" });
    const failed = exported(on("library"), {
      type: "Failed",
      reason: "The disk is full.",
    });

    expect(refused.notices).toEqual([]);
    expect(failed.notices).toEqual([]);
  });

  it("a PDF saved from the library still shows its notice", () => {
    const after = exported(on("library"), {
      type: "Saved",
      path: "/Users/candidate/Documents/Plate tectonics.pdf",
    });

    expect(after.notices).toMatchObject([
      {
        kind: "success",
        title: "PDF saved",
        description: "Plate tectonics.pdf",
      },
    ]);
  });
});

const BROWSER = "No application is set to open web links.";
const openWikipedia: CoreRequest = {
  type: "OpenExternal",
  url: "https://en.wikipedia.org/wiki/Asthenosphere",
};
const manageUsage: CoreRequest = { type: "OpenManageUsage" };

describe("the link notice", () => {
  it("a link the browser could not open shows an error notice with the core's reason", () => {
    const after = applyFailure(on("reader"), openWikipedia, {
      type: "Provider",
      message: BROWSER,
    });

    expect(after.notices).toEqual([
      {
        id: expect.any(String),
        kind: "error",
        title: "Link could not be opened",
        description: BROWSER,
        action: null,
      },
    ]);
  });

  it("Manage usage that could not be opened from the reader shows the same notice", () => {
    const after = applyFailure(on("reader"), manageUsage, { type: "Offline" });

    expect(after.notices).toEqual([
      {
        id: expect.any(String),
        kind: "error",
        title: "Link could not be opened",
        description: "The app is offline.",
        action: null,
      },
    ]);
  });

  it("a second link failure replaces the first instead of stacking", () => {
    const first = applyFailure(on("reader"), openWikipedia, {
      type: "Provider",
      message: BROWSER,
    });
    const second = applyFailure(first, manageUsage, { type: "Offline" });

    expect(second.notices).toHaveLength(1);
    expect(second.notices[0]).toMatchObject({
      title: "Link could not be opened",
      description: "The app is offline.",
    });
  });

  it("Manage usage from Settings or the sign-in screen, which say it in their own sentence, shows no notice", () => {
    for (const screen of ["settings", "signIn"] as const) {
      const after = applyFailure(on(screen), manageUsage, {
        type: "Provider",
        message: BROWSER,
      });

      expect(after.notices).toEqual([]);
    }
  });

  it("stays when other requests are answered, and goes when dismissed", () => {
    const failed = applyFailure(on("reader"), openWikipedia, {
      type: "Provider",
      message: BROWSER,
    });
    const later = applyResponse(
      applyResponse(failed, openWikipedia, { type: "Done" }),
      { type: "GetSession" },
      { type: "Session", session: signedIn },
    );

    expect(later.notices).toEqual(failed.notices);
    const dismissed = applyNavigation(later, {
      type: "DismissNotice",
      id: later.notices[0]!.id,
    });
    expect(dismissed.notices).toEqual([]);
  });
});

const LOCKED = "The database is locked by another process.";

const entry: GlossaryEntry = {
  id: "entry-1",
  courseId: "course-1",
  sectionId: "section-1",
  sectionTitle: "Convergent boundaries",
  term: "asthenosphere",
  definition: "The weak, partly molten layer of the mantle.",
  anchor: { versionId: "version-1", block: 0, quote: "asthenosphere" },
  wikipedia: null,
  createdAt: "2026-11-17T10:00:00.000Z",
};

const chat: SideChatSummary = {
  id: "chat-1",
  courseId: "course-1",
  anchor: null,
  sectionTitle: null,
  opening: "Why do plates move?",
  createdAt: "2026-11-17T10:00:00.000Z",
  updatedAt: "2026-11-17T10:00:00.000Z",
};

/** The reader with one definition and one side chat listed. */
function listed(): AtlasState {
  const withGlossary = applyResponse(
    on("reader"),
    { type: "ListGlossary", courseId: "course-1" },
    { type: "Glossary", courseId: "course-1", entries: [entry] },
  );
  return applyResponse(
    withGlossary,
    { type: "ListSideChats", courseId: "course-1" },
    { type: "SideChats", courseId: "course-1", chats: [chat] },
  );
}

const deleteEntry: CoreRequest = {
  type: "DeleteGlossaryEntry",
  entryId: entry.id,
};
const deleteChat: CoreRequest = { type: "DeleteSideChat", chatId: chat.id };

describe("the delete notices", () => {
  it("a definition that could not be deleted shows an error notice with the reason, and stays", () => {
    const before = listed();
    const after = applyFailure(before, deleteEntry, {
      type: "Store",
      message: LOCKED,
    });

    expect(after.notices).toEqual([
      {
        id: expect.any(String),
        kind: "error",
        title: "Definition could not be deleted",
        description: LOCKED,
        action: null,
      },
    ]);
    expect(after.glossary).toEqual(before.glossary);
    expect(after.glossary.byCourse["course-1"]).toEqual([entry]);
  });

  it("a side chat that could not be deleted shows an error notice with the reason, and stays", () => {
    const before = listed();
    const after = applyFailure(before, deleteChat, {
      type: "Store",
      message: LOCKED,
    });

    expect(after.notices).toEqual([
      {
        id: expect.any(String),
        kind: "error",
        title: "Side chat could not be deleted",
        description: LOCKED,
        action: null,
      },
    ]);
    expect(after.sideChats).toEqual(before.sideChats);
    expect(after.sideChats.byCourse["course-1"]).toEqual([chat]);
    expect(after.navigation).toEqual(before.navigation);
  });

  it("each cause has its own notice, and a repeat replaces its own", () => {
    const failed = applyFailure(
      applyFailure(
        applyFailure(listed(), deleteEntry, { type: "Offline" }),
        deleteChat,
        { type: "Offline" },
      ),
      deleteEntry,
      { type: "Store", message: LOCKED },
    );

    expect(
      failed.notices.map(({ title, description }) => [title, description]),
    ).toEqual([
      ["Definition could not be deleted", LOCKED],
      ["Side chat could not be deleted", "The app is offline."],
    ]);
  });

  it("a delete that succeeds on retry closes its own failure notice", () => {
    const failed = applyFailure(
      applyFailure(listed(), deleteEntry, { type: "Offline" }),
      deleteChat,
      { type: "Store", message: LOCKED },
    );

    const entryGone = applyResponse(failed, deleteEntry, { type: "Done" });
    expect(entryGone.notices.map(({ title }) => title)).toEqual([
      "Side chat could not be deleted",
    ]);

    const bothGone = applyResponse(entryGone, deleteChat, { type: "Done" });
    expect(bothGone.notices).toEqual([]);
  });
});

describe("outcomes the screen already shows", () => {
  it("signing out, deleting a course, a definition or a side chat show no notice", () => {
    const signedOut = applyResponse(
      listed(),
      { type: "SignOut" },
      {
        type: "Session",
        session: { type: "SignedOut", reason: { type: "SignedOut" } },
      },
    );
    const deleted = [
      { type: "DeleteCourse", courseId: "course-1" } as const,
      deleteEntry,
      deleteChat,
    ].reduce(
      (state, request) => applyResponse(state, request, { type: "Done" }),
      listed(),
    );

    expect(signedOut.notices).toEqual([]);
    expect(deleted.notices).toEqual([]);
    expect(deleted.glossary.byCourse["course-1"]).toEqual([]);
    expect(deleted.sideChats.byCourse["course-1"]).toEqual([]);
  });
});
