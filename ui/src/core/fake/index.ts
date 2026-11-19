import { createCoreClient, type CoreClient } from "../client";
import type { CoreError } from "../generated/CoreError";
import type { CoreEvent } from "../generated/CoreEvent";
import type { CoreRequest } from "../generated/CoreRequest";
import type { CoreResponse } from "../generated/CoreResponse";
import type { CourseSummary } from "../generated/CourseSummary";
import type { CourseView } from "../generated/CourseView";
import type { GenerationEvent } from "../generated/GenerationEvent";
import type { SectionNode } from "../generated/SectionNode";
import type { SectionView } from "../generated/SectionView";
import type { FakeClock } from "./clock";

export { createManualClock, systemClock, type FakeClock } from "./clock";

/** The scenarios found in `scenarios/`, by name. */
export const scenarioNames: string[] = [];

/** What the fake answers: the index of every new course and the text of every section. */
export interface FakeScript {
  chapters: { title: string; sections: { title: string; summary: string }[] }[];
  sectionDeltas: string[];
}

export interface FakeCoreOptions {
  script?: FakeScript;
  scenario?: string;
  clock?: FakeClock;
  /** Awaited before each streamed delta; sets the pace of generation. */
  pace?: () => Promise<void>;
}

export const sampleScript: FakeScript = {
  chapters: [
    {
      title: "Foundations",
      sections: [
        { title: "What it is", summary: "The territory in one page." },
        { title: "Why it matters", summary: "Where the topic shows up." },
      ],
    },
    {
      title: "Core ideas",
      sections: [
        {
          title: "The first principle",
          summary: "The idea everything rests on.",
        },
        {
          title: "The second principle",
          summary: "How the first one is used.",
        },
      ],
    },
    {
      title: "Going further",
      sections: [
        { title: "Open questions", summary: "What is still debated." },
        { title: "Beyond the basics", summary: "The deep dives worth taking." },
      ],
    },
  ],
  sectionDeltas: [
    "This section opens with the main idea. ",
    "It then builds on it step by step. ",
    "It closes with what comes next.",
  ],
};

function delay(ms: number): () => Promise<void> {
  return () => {
    const { promise, resolve } = Promise.withResolvers<void>();
    setTimeout(resolve, ms);
    return promise;
  };
}

interface StoredCourse {
  summary: CourseSummary;
  view: CourseView;
  /** The view's sections in index order. */
  sections: SectionNode[];
  /** Each generated section, as it is reopened. */
  stored: Map<string, SectionView>;
}

type Built =
  "ListCourses" | "CreateCourse" | "OpenCourse" | "OpenSection" | "Next";

/** The feature folder that answers each request the core does not build yet. */
const features: Record<Exclude<CoreRequest["type"], Built>, string> = {
  GetSession: "session",
  SignIn: "session",
  CancelSignIn: "session",
  EnablePlanUsage: "session",
  SignOut: "session",
  OpenManageUsage: "external",
  OpenExternal: "external",
  ListModels: "models",
  SelectModel: "models",
  DeleteCourse: "library",
  SubmitSelfRating: "placement",
  AnswerPlacement: "placement",
  RetryPlacementStep: "placement",
  StopGeneration: "sections",
  RetrySection: "sections",
  ExplainDifferently: "versions",
  SelectVersion: "versions",
  AnswerCheck: "checks",
  GoDeeper: "deep_dives",
  Define: "define",
  LookupWikipedia: "wikipedia",
  ListGlossary: "define",
  ListSideChats: "side_chats",
  StartSideChat: "side_chats",
  OpenSideChat: "side_chats",
  SendSideChatMessage: "side_chats",
  StopSideChatReply: "side_chats",
  GetUsage: "usage",
  ConnectUsage: "usage",
  CancelConnectUsage: "usage",
  SetWindowFocused: "usage",
  GetExportDocument: "export",
  ExportPdf: "export",
};

/**
 * An in-memory core that answers like the real one, for tests and for Vite's
 * `fake` mode. It is never part of the production bundle.
 */
export function createFakeCore(options: FakeCoreOptions = {}): CoreClient {
  const script = options.script ?? sampleScript;
  const pace = options.pace ?? delay(150);
  const courses = new Map<string, StoredCourse>();
  const listeners = new Set<(event: CoreEvent) => void>();

  const emit = (event: CoreEvent) => {
    for (const listener of listeners) listener(event);
  };
  const fail = (error: CoreError): never => {
    throw error;
  };
  const course = (id: string) =>
    courses.get(id) ?? fail({ type: "NotFound", what: `course ${id}` });

  async function openSection(
    stored: StoredCourse,
    sectionId: string,
  ): Promise<SectionView> {
    const entry =
      stored.sections.find((section) => section.id === sectionId) ??
      fail({ type: "NotFound", what: `section ${sectionId}` });
    const courseId = stored.summary.id;
    const existing = stored.stored.get(sectionId);
    if (existing !== undefined) return structuredClone(existing);

    const generationId = crypto.randomUUID();
    const send = (event: GenerationEvent) =>
      emit({
        type: "Generation",
        generationId,
        target: { type: "Section", courseId, sectionId, style: "Original" },
        event,
      });
    send({ type: "Started" });
    send({ type: "BlockOpened", block: 0, kind: "Prose" });
    let text = "";
    for (const delta of script.sectionDeltas) {
      await pace();
      text += delta;
      send({ type: "TextDelta", block: 0, text: delta });
    }
    const block = { index: 0, content: { type: "Prose" as const, text } };
    send({ type: "BlockCompleted", block });
    const versionId = crypto.randomUUID();
    const createdAt = new Date().toISOString();
    const section: SectionView = {
      id: entry.id,
      courseId,
      title: entry.title,
      summary: entry.summary,
      version: {
        id: versionId,
        style: "Original",
        blocks: [block],
        citations: [],
        sources: [],
        writtenWithoutSearch: false,
        model: null,
        createdAt,
      },
      versions: [{ id: versionId, style: "Original", createdAt }],
    };
    stored.stored.set(sectionId, section);
    entry.read = true;
    entry.mark = "Read";
    stored.summary.progress.read += 1;
    send({ type: "Finished", outcome: { type: "Completed" } });
    return structuredClone(section);
  }

  async function handle(request: CoreRequest): Promise<CoreResponse> {
    switch (request.type) {
      case "ListCourses":
        return {
          type: "Courses",
          courses: [...courses.values()]
            .map((stored) => stored.summary)
            .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)),
        };
      case "CreateCourse": {
        const topic = request.topic.trim();
        if (topic === "") {
          fail({ type: "InvalidRequest", message: "the topic is empty" });
        }
        const now = new Date().toISOString();
        const id = crypto.randomUUID();
        const view: CourseView = {
          id,
          topic,
          stage: "Ready",
          chapters: script.chapters.map((chapter) => ({
            id: crypto.randomUUID(),
            title: chapter.title,
            sections: chapter.sections.map((section) => ({
              id: crypto.randomUUID(),
              kind: "BroadPass",
              title: section.title,
              summary: section.summary,
              read: false,
              mark: "Unread",
              deepDives: [],
            })),
          })),
          resumeAt: null,
          glossaryCount: 0,
          sideChatCount: 0,
          placement: null,
        };
        const sections = view.chapters.flatMap((chapter) => chapter.sections);
        courses.set(id, {
          summary: {
            id,
            topic,
            stage: "Ready",
            progress: { read: 0, total: sections.length },
            resumeTitle: null,
            lastOpenedAt: null,
            createdAt: now,
            updatedAt: now,
          },
          view,
          sections,
          stored: new Map(),
        });
        return { type: "Course", course: structuredClone(view) };
      }
      case "OpenCourse":
        return {
          type: "Course",
          course: structuredClone(course(request.courseId).view),
        };
      case "OpenSection":
        return {
          type: "Section",
          section: await openSection(
            course(request.courseId),
            request.sectionId,
          ),
        };
      case "Next": {
        const stored = course(request.courseId);
        const sections = stored.sections;
        const position = sections.findIndex(
          (section) => section.id === request.sectionId,
        );
        if (position === -1) {
          fail({ type: "NotFound", what: `section ${request.sectionId}` });
        }
        const following = sections[position + 1];
        if (following === undefined) {
          return { type: "EndOfCourse", courseId: request.courseId };
        }
        return {
          type: "Section",
          section: await openSection(stored, following.id),
        };
      }
      default:
        return fail({ type: "Unavailable", feature: features[request.type] });
    }
  }

  return createCoreClient({
    request: handle,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  });
}
