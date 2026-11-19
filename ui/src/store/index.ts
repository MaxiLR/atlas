import type { CoreError } from "@/core/generated/CoreError";
import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CoreResponse } from "@/core/generated/CoreResponse";
import type { CourseView } from "@/core/generated/CourseView";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";
import type { PlacementState } from "@/core/generated/PlacementState";
import type { Session } from "@/core/generated/Session";
import type { SideChatSummary } from "@/core/generated/SideChatSummary";
import type { UsageState } from "@/core/generated/UsageState";

import {
  courseChanged,
  courseOpened,
  initialCourse,
  withDeepDives,
  type CourseSlice,
} from "./course";
import {
  definitionGenerated,
  entryChanged,
  glossaryListed,
  initialGlossary,
  wikipediaLookedUp,
  type GlossarySlice,
} from "./glossary";
import {
  courseDeleted,
  coursesListed,
  initialLibrary,
  type LibrarySlice,
} from "./library";
import { initialModels, type ModelsSlice } from "./models";
import {
  courseNavigated,
  initialNavigation,
  navigate,
  placementNavigated,
  sectionNavigated,
  sessionNavigated,
  type NavigationAction,
  type NavigationSlice,
} from "./navigation";
import {
  initialPlacement,
  placementChanged,
  type PlacementSlice,
} from "./placement";
import {
  checkAnswered,
  initialSections,
  sectionAnswered,
  sectionDisplay,
  sectionFailed,
  sectionGenerated,
  type SectionDisplay,
  type SectionsSlice,
} from "./sections";
import { initialSession, type SessionSlice } from "./session";
import {
  chatChanged,
  chatOpened,
  chatsListed,
  followUpsOffered,
  initialSideChats,
  messageSent,
  replied,
  replyGenerated,
  type SideChatsSlice,
} from "./side-chats";
import { unknownTag } from "./unknown-tag";
import { initialUsage, usageChanged, type UsageSlice } from "./usage";

export type { SectionDisplay } from "./sections";
export type { ShownBlock, Stream } from "./stream";
export type { NavigationAction, PanelTab, Screen } from "./navigation";

/** Everything the interface shows, fed by the core's events and responses. */
export interface AtlasState {
  session: SessionSlice;
  models: ModelsSlice;
  library: LibrarySlice;
  placement: PlacementSlice;
  course: CourseSlice;
  sections: SectionsSlice;
  glossary: GlossarySlice;
  sideChats: SideChatsSlice;
  usage: UsageSlice;
  navigation: NavigationSlice;
}

export const initialState: AtlasState = {
  session: initialSession,
  models: initialModels,
  library: initialLibrary,
  placement: initialPlacement,
  course: initialCourse,
  sections: initialSections,
  glossary: initialGlossary,
  sideChats: initialSideChats,
  usage: initialUsage,
  navigation: initialNavigation,
};

function sessionChanged(state: AtlasState, session: Session): AtlasState {
  return {
    ...state,
    session: { session },
    navigation: sessionNavigated(state.navigation, session),
  };
}

function placementProgressed(
  state: AtlasState,
  courseId: string,
  placement: PlacementState,
): AtlasState {
  const done = placement.step.type === "Done";
  return {
    ...state,
    placement: placementChanged(state.placement, courseId, placement),
    course: courseChanged(state.course, courseId, (course) => ({
      ...course,
      stage: done ? "Ready" : course.stage,
      placement: done ? null : placement,
    })),
    navigation: placementNavigated(state.navigation, courseId, placement),
  };
}

function courseLoaded(state: AtlasState, course: CourseView): AtlasState {
  return {
    ...state,
    course: courseOpened(state.course, course),
    placement:
      course.placement === null
        ? state.placement
        : placementChanged(state.placement, course.id, course.placement),
    navigation: courseNavigated(state.navigation, course),
  };
}

/** A glossary entry arrived; a new one counts towards its course. */
function glossaryEntryArrived(
  state: AtlasState,
  entry: GlossaryEntry,
): AtlasState {
  const known = (state.glossary.byCourse[entry.courseId] ?? []).some(
    (existing) => existing.id === entry.id,
  );
  return {
    ...state,
    glossary: entryChanged(state.glossary, entry),
    course: known
      ? state.course
      : courseChanged(state.course, entry.courseId, (course) => ({
          ...course,
          glossaryCount: course.glossaryCount + 1,
        })),
  };
}

function sideChatCounted(state: AtlasState, courseId: string): AtlasState {
  return {
    ...state,
    course: courseChanged(state.course, courseId, (course) => ({
      ...course,
      sideChatCount: course.sideChatCount + 1,
    })),
  };
}

/**
 * A side chat was started or received a message. On a course whose chats
 * are listed, one not among them is new and counts; on any other the event
 * cannot tell, and the answer to starting it counts it instead.
 */
function sideChatArrived(
  state: AtlasState,
  summary: SideChatSummary,
): AtlasState {
  const { courseId } = summary;
  const isNew =
    state.sideChats.listed[courseId] === true &&
    !(state.sideChats.byCourse[courseId] ?? []).some(
      (chat) => chat.id === summary.id,
    );
  const changed = {
    ...state,
    sideChats: chatChanged(state.sideChats, summary),
  };
  return isNew ? sideChatCounted(changed, courseId) : changed;
}

function generated(
  state: AtlasState,
  event: Extract<CoreEvent, { type: "Generation" }>,
): AtlasState {
  const { generationId, target } = event;
  switch (target.type) {
    case "Section": {
      const nav = state.navigation;
      const followsNext =
        event.event.type === "Started" &&
        nav.nextFrom !== null &&
        nav.courseId === target.courseId;
      return {
        ...state,
        sections: sectionGenerated(
          state.sections,
          target,
          generationId,
          event.event,
        ),
        navigation: followsNext
          ? sectionNavigated(nav, target.courseId, target.sectionId)
          : nav,
      };
    }
    case "Define":
      return {
        ...state,
        glossary: definitionGenerated(
          state.glossary,
          target,
          generationId,
          event.event,
        ),
      };
    case "SideChat":
      return {
        ...state,
        sideChats: replyGenerated(
          state.sideChats,
          target.chatId,
          generationId,
          event.event,
        ),
      };
    default:
      return unknownTag(target);
  }
}

/** The state after one core event. */
export function applyEvent(state: AtlasState, event: CoreEvent): AtlasState {
  switch (event.type) {
    case "SessionChanged":
      return sessionChanged(state, event.session);
    case "ModelsChanged":
      return { ...state, models: { list: event.list } };
    case "CoursesChanged":
      return { ...state, library: coursesListed(event.courses) };
    case "IndexChanged":
      return {
        ...state,
        course: courseChanged(state.course, event.courseId, (course) => ({
          ...course,
          chapters: event.chapters,
        })),
      };
    case "PlacementProgress":
      return placementProgressed(state, event.courseId, event.placement);
    case "Generation":
      return generated(state, event);
    case "GlossaryChanged":
      return glossaryEntryArrived(state, event.entry);
    case "SideChatsChanged":
      return sideChatArrived(state, event.chat);
    case "FollowUpPrompts":
      return {
        ...state,
        sideChats: followUpsOffered(
          state.sideChats,
          event.chatId,
          event.prompts,
        ),
      };
    case "UsageChanged":
      return { ...state, usage: usageChanged(state.usage, event.usage) };
    default:
      return unknownTag(event);
  }
}

/** The state once `request` has been sent, before the core answers. */
export function applyRequest(
  state: AtlasState,
  request: CoreRequest,
): AtlasState {
  switch (request.type) {
    case "OpenSection":
      return {
        ...state,
        sections: sectionFailed(
          state.sections,
          request.courseId,
          request.sectionId,
          null,
        ),
        navigation: sectionNavigated(
          state.navigation,
          request.courseId,
          request.sectionId,
        ),
      };
    case "RetrySection":
    case "ExplainDifferently":
    case "SelectVersion":
      return {
        ...state,
        sections: sectionFailed(
          state.sections,
          request.courseId,
          request.sectionId,
          null,
        ),
      };
    case "Next":
      return {
        ...state,
        navigation: {
          ...state.navigation,
          nextFrom: request.sectionId,
          endOfCourse: false,
        },
      };
    case "ListCourses":
    case "CreateCourse":
    case "OpenCourse":
    case "DeleteCourse":
      return { ...state, library: { ...state.library, error: null } };
    case "SendSideChatMessage":
      return {
        ...state,
        sideChats: messageSent(state.sideChats, request.chatId, request.text),
      };
    case "GetSession":
    case "SignIn":
    case "CancelSignIn":
    case "EnablePlanUsage":
    case "SignOut":
    case "OpenManageUsage":
    case "OpenExternal":
    case "ListModels":
    case "SelectModel":
    case "SubmitSelfRating":
    case "AnswerPlacement":
    case "RetryPlacementStep":
    case "StopGeneration":
    case "AnswerCheck":
    case "GoDeeper":
    case "Define":
    case "LookupWikipedia":
    case "ListGlossary":
    case "ListSideChats":
    case "StartSideChat":
    case "OpenSideChat":
    case "StopSideChatReply":
    case "GetUsage":
    case "ConnectUsage":
    case "CancelConnectUsage":
    case "SetWindowFocused":
    case "GetExportDocument":
    case "ExportPdf":
      return state;
    default:
      return unknownTag(request);
  }
}

/** Where Next leads, unless the candidate went elsewhere meanwhile. */
function nextLed(
  state: AtlasState,
  request: CoreRequest,
  courseId: string,
  sectionId: string,
): NavigationSlice {
  const nav = state.navigation;
  return request.type === "Next" && nav.nextFrom === request.sectionId
    ? sectionNavigated(nav, courseId, sectionId)
    : nav;
}

/** The state after the core answered `request` with `response`. */
export function applyResponse(
  state: AtlasState,
  request: CoreRequest,
  response: CoreResponse,
): AtlasState {
  switch (response.type) {
    case "Session":
      return sessionChanged(state, response.session);
    case "Models":
      return { ...state, models: { list: response.list } };
    case "Courses":
      return { ...state, library: coursesListed(response.courses) };
    case "Course":
      return courseLoaded(state, response.course);
    case "Placement":
      return placementProgressed(state, response.courseId, response.placement);
    case "Section":
      return {
        ...state,
        sections: sectionAnswered(
          state.sections,
          response.section,
          request.type === "SelectVersion",
        ),
        navigation: nextLed(
          state,
          request,
          response.section.courseId,
          response.section.id,
        ),
      };
    case "NextSection":
      return {
        ...state,
        navigation: nextLed(
          state,
          request,
          response.courseId,
          response.sectionId,
        ),
      };
    case "EndOfCourse":
      return {
        ...state,
        navigation: { ...state.navigation, nextFrom: null, endOfCourse: true },
      };
    case "DeepDives":
      return {
        ...state,
        course: courseChanged(state.course, response.courseId, (course) => ({
          ...course,
          chapters: withDeepDives(
            course.chapters,
            response.sectionId,
            response.deepDives,
          ),
        })),
      };
    case "CheckAnswered":
      return request.type === "AnswerCheck"
        ? {
            ...state,
            sections: checkAnswered(state.sections, request, response.result),
          }
        : state;
    case "Definition":
      return glossaryEntryArrived(state, response.entry);
    case "Wikipedia":
      return {
        ...state,
        glossary: wikipediaLookedUp(
          state.glossary,
          response.entryId,
          response.result,
        ),
      };
    case "Glossary":
      return {
        ...state,
        glossary: glossaryListed(
          state.glossary,
          response.courseId,
          response.entries,
        ),
        course: courseChanged(state.course, response.courseId, (course) => ({
          ...course,
          glossaryCount: response.entries.length,
        })),
      };
    case "SideChats":
      return {
        ...state,
        sideChats: chatsListed(
          state.sideChats,
          response.courseId,
          response.chats,
        ),
        course: courseChanged(state.course, response.courseId, (course) => ({
          ...course,
          sideChatCount: response.chats.length,
        })),
      };
    case "SideChat": {
      const opened = {
        ...state,
        sideChats: chatOpened(state.sideChats, response.chat),
        navigation: {
          ...state.navigation,
          panel: {
            open: true,
            tab: "sideChats" as const,
            chatId: response.chat.id,
          },
        },
      };
      const { courseId } = response.chat;
      return request.type === "StartSideChat" &&
        state.sideChats.listed[courseId] !== true
        ? sideChatCounted(opened, courseId)
        : opened;
    }
    case "SideChatReply":
      return {
        ...state,
        sideChats: replied(state.sideChats, response.chatId, response.message),
      };
    case "Usage":
      return { ...state, usage: usageChanged(state.usage, response.usage) };
    case "Done":
      return request.type === "DeleteCourse"
        ? { ...state, library: courseDeleted(state.library, request.courseId) }
        : state;
    case "ExportDocument":
    case "Exported":
      return state;
    default:
      return unknownTag(response);
  }
}

/** The state after the core refused `request`. */
export function applyFailure(
  state: AtlasState,
  request: CoreRequest,
  error: CoreError,
): AtlasState {
  switch (request.type) {
    case "ListCourses":
    case "CreateCourse":
    case "OpenCourse":
    case "DeleteCourse":
      return { ...state, library: { ...state.library, error } };
    case "OpenSection":
    case "RetrySection":
    case "ExplainDifferently":
    case "SelectVersion":
      return {
        ...state,
        sections: sectionFailed(
          state.sections,
          request.courseId,
          request.sectionId,
          error,
        ),
      };
    case "Next":
      return {
        ...state,
        sections: sectionFailed(
          state.sections,
          request.courseId,
          request.sectionId,
          error,
        ),
        navigation: { ...state.navigation, nextFrom: null },
      };
    case "GetSession":
    case "SignIn":
    case "CancelSignIn":
    case "EnablePlanUsage":
    case "SignOut":
    case "OpenManageUsage":
    case "OpenExternal":
    case "ListModels":
    case "SelectModel":
    case "SubmitSelfRating":
    case "AnswerPlacement":
    case "RetryPlacementStep":
    case "StopGeneration":
    case "AnswerCheck":
    case "GoDeeper":
    case "Define":
    case "LookupWikipedia":
    case "ListGlossary":
    case "ListSideChats":
    case "StartSideChat":
    case "OpenSideChat":
    case "SendSideChatMessage":
    case "StopSideChatReply":
    case "GetUsage":
    case "ConnectUsage":
    case "CancelConnectUsage":
    case "SetWindowFocused":
    case "GetExportDocument":
    case "ExportPdf":
      return state;
    default:
      return unknownTag(request);
  }
}

/** The state after an action of the interface's own. */
export function applyNavigation(
  state: AtlasState,
  action: NavigationAction,
): AtlasState {
  return { ...state, navigation: navigate(state.navigation, action) };
}

/** A section as the reading column shows it; null before it was opened. */
export function selectSection(
  state: AtlasState,
  sectionId: string,
): SectionDisplay | null {
  return sectionDisplay(state.sections, sectionId);
}

/** The course open in the window, once the core has sent it. */
export function selectOpenCourse(state: AtlasState): CourseView | null {
  const { courseId } = state.navigation;
  return courseId === null ? null : (state.course.byId[courseId] ?? null);
}

/** The glossary terms and side chats anchored to the version a section shows. */
export function selectAnchors(
  state: AtlasState,
  sectionId: string,
): { terms: GlossaryEntry[]; chats: SideChatSummary[] } {
  const section = state.sections.byId[sectionId];
  const versionId = sectionDisplay(state.sections, sectionId)?.versionId;
  if (section === undefined || versionId == null) {
    return { terms: [], chats: [] };
  }
  return {
    terms: (state.glossary.byCourse[section.courseId] ?? []).filter(
      (entry) => entry.anchor.versionId === versionId,
    ),
    chats: (state.sideChats.byCourse[section.courseId] ?? []).filter(
      (chat) => chat.anchor?.versionId === versionId,
    ),
  };
}

/** A course's side chats, newest first; null until the core has listed them. */
export function selectSideChats(
  state: AtlasState,
  courseId: string,
): SideChatSummary[] | null {
  return state.sideChats.listed[courseId] === true
    ? (state.sideChats.byCourse[courseId] ?? [])
    : null;
}

/** The usage meter: the last reading stays, stale, when a later one fails. */
export function selectUsage(state: AtlasState): UsageState | null {
  return state.usage.state;
}
