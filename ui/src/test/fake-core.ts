import { createAtlasStore, type AtlasStore } from "@/app/store";
import {
  createFakeCore,
  createManualClock,
  type FakeCoreOptions,
} from "@/core/fake";
import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { CourseView } from "@/core/generated/CourseView";
import type { SectionNode } from "@/core/generated/SectionNode";

/** The time the manual clock of `startFake` starts at. */
export const startedAt = "2026-11-19T10:00:00.000Z";

/**
 * A pace a test holds: `step()` lets one waiting step through, `open()` lets
 * every step through until `close()`.
 */
export function createValve() {
  let open = false;
  const waiting: (() => void)[] = [];
  return {
    pace(): Promise<void> {
      if (open) return Promise.resolve();
      const { promise, resolve } = Promise.withResolvers<void>();
      waiting.push(resolve);
      return promise;
    },
    step() {
      waiting.shift()?.();
    },
    open() {
      open = true;
      for (const release of waiting.splice(0)) release();
    },
    close() {
      open = false;
    },
  };
}

/**
 * Lets every promise the fake core has queued settle: a negative check (no
 * reading, no sign-in) needs the core to have run as far as it can. A
 * zero-delay task runs only after all queued microtasks.
 */
export function drained(): Promise<void> {
  const { promise, resolve } = Promise.withResolvers<void>();
  setTimeout(resolve, 0);
  return promise;
}

/**
 * The fake core in `scenario` behind a connected store, on a manual clock,
 * with every step it takes let through at once unless `pace` says otherwise.
 * `events` records every event the core emits.
 */
export function startFake(
  scenario = "default",
  options: Partial<FakeCoreOptions> = {},
) {
  const clock = createManualClock(startedAt);
  const core = createFakeCore({
    scenario,
    clock,
    pace: () => Promise.resolve(),
    ...options,
  });
  const store = createAtlasStore(core);
  store.connect();
  const events: CoreEvent[] = [];
  core.subscribe((event) => events.push(event));
  return { core, store, clock, events, state: () => store.state.getState() };
}

/** Opens the library's course on `topic`; returns its view. */
export async function openCourse(
  store: AtlasStore,
  topic: string,
): Promise<CourseView> {
  const { courses } = await store.send({ type: "ListCourses" });
  const summary = courses.find((course) => course.topic === topic);
  if (summary === undefined) throw new Error(`no course on ${topic}`);
  const { course } = await store.send({
    type: "OpenCourse",
    courseId: summary.id,
  });
  return course;
}

/** The section titled `title` anywhere in the course's index tree. */
export function findSection(course: CourseView, title: string): SectionNode {
  const search = (sections: SectionNode[]): SectionNode | undefined => {
    for (const section of sections) {
      if (section.title === title) return section;
      const deeper = search(section.deepDives);
      if (deeper !== undefined) return deeper;
    }
    return undefined;
  };
  const found = search(course.chapters.flatMap((chapter) => chapter.sections));
  if (found === undefined) throw new Error(`no section ${title}`);
  return found;
}

/**
 * Starts a course on `topic` and answers placement through to its index,
 * picking the first option of every question; returns the course as the
 * store holds it.
 */
export async function placeCourse(
  store: AtlasStore,
  topic: string,
): Promise<CourseView> {
  const { course } = await store.send({ type: "CreateCourse", topic });
  let { placement } = await store.send({
    type: "SubmitSelfRating",
    courseId: course.id,
    rating: "KnowTheBasics",
    goal: "WorkingKnowledge",
  });
  while (placement.step.type === "Question") {
    ({ placement } = await store.send({
      type: "AnswerPlacement",
      courseId: course.id,
      questionId: placement.step.question.id,
      answer: { type: "Option", index: 0 },
    }));
  }
  const placed = store.state.getState().course.byId[course.id];
  if (placed === undefined) throw new Error("the course left the store");
  return placed;
}
