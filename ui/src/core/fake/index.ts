import type { CoreClient } from "../client";
import type { CoreError } from "../generated/CoreError";
import type { CoreEvent } from "../generated/CoreEvent";
import type { CoreRequest } from "../generated/CoreRequest";
import type { CoreResponse } from "../generated/CoreResponse";
import type { CourseSummary } from "../generated/CourseSummary";
import type { CourseView } from "../generated/CourseView";
import type { SectionEntry } from "../generated/SectionEntry";
import type { SectionView } from "../generated/SectionView";

/** What the fake answers: the index of every new course and the text of every section. */
export interface FakeScript {
  chapters: { title: string; sections: { title: string; summary: string }[] }[];
  sectionDeltas: string[];
}

export interface FakeCoreOptions {
  script?: FakeScript;
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
  sections: SectionEntry[];
  contents: Map<string, string>;
}

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
    let content = stored.contents.get(sectionId);
    if (content === undefined) {
      content = "";
      for (const text of script.sectionDeltas) {
        await pace();
        content += text;
        emit({ type: "SectionDelta", courseId, sectionId, text });
      }
      stored.contents.set(sectionId, content);
      entry.generated = true;
      emit({ type: "SectionCompleted", courseId, sectionId });
    }
    return { id: entry.id, courseId, title: entry.title, content };
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
          chapters: script.chapters.map((chapter) => ({
            id: crypto.randomUUID(),
            title: chapter.title,
            sections: chapter.sections.map((section) => ({
              id: crypto.randomUUID(),
              title: section.title,
              summary: section.summary,
              generated: false,
            })),
          })),
        };
        courses.set(id, {
          summary: { id, topic, createdAt: now, updatedAt: now },
          view,
          sections: view.chapters.flatMap((chapter) => chapter.sections),
          contents: new Map(),
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
    }
  }

  return {
    call: handle,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
