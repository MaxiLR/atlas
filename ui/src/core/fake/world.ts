/**
 * What the fake core holds: the session, the library with each course's
 * index, versions, glossary and side chats, the usage meter, and the
 * conditions a scenario sets (offline, a refused search, a failing
 * generation). Scenarios build it; the core reads and changes it.
 */
import type { Anchor } from "../generated/Anchor";
import type { Block } from "../generated/Block";
import type { BlockKind } from "../generated/BlockKind";
import type { ChapterNode } from "../generated/ChapterNode";
import type { Citation } from "../generated/Citation";
import type { CoreError } from "../generated/CoreError";
import type { CourseStage } from "../generated/CourseStage";
import type { CourseSummary } from "../generated/CourseSummary";
import type { CourseView } from "../generated/CourseView";
import type { GlossaryEntry } from "../generated/GlossaryEntry";
import type { Goal } from "../generated/Goal";
import type { ModelList } from "../generated/ModelList";
import type { PlacementAnswer } from "../generated/PlacementAnswer";
import type { PlacementState } from "../generated/PlacementState";
import type { PlanUsage } from "../generated/PlanUsage";
import type { SectionKind } from "../generated/SectionKind";
import type { SectionNode } from "../generated/SectionNode";
import type { SectionVersion } from "../generated/SectionVersion";
import type { SectionView } from "../generated/SectionView";
import type { SelfRating } from "../generated/SelfRating";
import type { Session } from "../generated/Session";
import type { SideChat } from "../generated/SideChat";
import type { SideChatSummary } from "../generated/SideChatSummary";
import type { UsageFailure } from "../generated/UsageFailure";
import type { UsageLevel } from "../generated/UsageLevel";
import type { UsageReading } from "../generated/UsageReading";
import type { UsageState } from "../generated/UsageState";
import type { VersionStyle } from "../generated/VersionStyle";
import type { FakeClock } from "./clock";
import {
  definitions,
  followUps,
  frenchRevolutionQuestions,
  sampleIndex,
  sideChatQuestion,
  sideChatReply,
  type PlacementDraft,
  type SampleBlock,
  type SampleSection,
} from "./sample-course";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export interface FakeSection {
  id: string;
  courseId: string;
  kind: SectionKind;
  sample: SampleSection;
  /** Stored versions, oldest first. */
  versions: SectionVersion[];
  currentVersionId: string | null;
  read: boolean;
  /** Null until Go deeper has proposed them. */
  deepDives: FakeSection[] | null;
  parent: FakeSection | null;
}

export interface FakePlacement {
  state: PlacementState;
  /** The questions it asks, in order; the index is built after the last. */
  questions: PlacementDraft[];
  rating: SelfRating | null;
  goal: Goal | null;
  answers: PlacementAnswer[];
  /** The id of the question shown, while one is. */
  questionId: string | null;
}

export interface FakeCourse {
  id: string;
  topic: string;
  stage: CourseStage;
  chapters: { id: string; title: string; sections: FakeSection[] }[];
  placement: FakePlacement | null;
  /** The section last opened. */
  resumeAt: string | null;
  glossary: GlossaryEntry[];
  /** Newest first. */
  chats: SideChat[];
  createdAt: string;
  updatedAt: string;
  lastOpenedAt: string | null;
}

/** How every generation of a scenario ends instead of completing. */
export type GenerationFault =
  | { type: "Stop"; once: boolean }
  | { type: "Fail"; error: CoreError; retries: number };

/** A placement step that fails once, then works when retried. */
export interface PlacementFailure {
  step: "PreparingQuestions" | "BuildingIndex";
  error: CoreError;
}

export interface FakeWorld {
  clock: FakeClock;
  session: Session;
  /** What plan usage a completed sign-in grants. */
  grantedPlanUsage: PlanUsage;
  models: ModelList;
  courses: FakeCourse[];
  usage: UsageState;
  /** Share used of the 5-hour, weekly and monthly windows. */
  usageShares: [number, number, number];
  /** Why every usage reading fails, when it does. */
  usageFailure: UsageFailure | null;
  /** Anything needing a request says the app is offline. */
  offline: boolean;
  /** The account's policy refuses web search. */
  searchRefused: boolean;
  fault: GenerationFault | null;
  /** How many questions placement asks a new course, four to six. */
  placementQuestions: number;
  placementFailure: PlacementFailure | null;
}

export const account = { email: "candidate@example.com" };

export function createWorld(clock: FakeClock): FakeWorld {
  return {
    clock,
    session: { type: "SignedOut", reason: null },
    grantedPlanUsage: "On",
    models: {
      models: [
        { slug: "gpt-5.5", displayName: "GPT-5.5" },
        { slug: "gpt-5.4", displayName: "GPT-5.4" },
        { slug: "gpt-5.4-mini", displayName: "GPT-5.4 mini" },
      ],
      current: "gpt-5.5",
    },
    courses: [],
    usage: { type: "NotConnected" },
    usageShares: [0.38, 0.61, 0.12],
    usageFailure: null,
    offline: false,
    searchRefused: false,
    fault: null,
    placementQuestions: 4,
    placementFailure: null,
  };
}

export function iso(world: FakeWorld, offset = 0): string {
  return new Date(world.clock.now().getTime() + offset).toISOString();
}

/** Signed in with ChatGPT, plan usage as given. */
export function signIn(world: FakeWorld, planUsage: PlanUsage = "On") {
  world.grantedPlanUsage = planUsage;
  world.session = { type: "SignedIn", account, planUsage };
}

// -- Blocks and versions ------------------------------------------------------

export function kindOf(block: SampleBlock): BlockKind {
  return block.type === "Fallback" ? "Visual" : block.type;
}

export function proseText(block: SampleBlock & { type: "Prose" }): string {
  return block.chunks.join("");
}

/** The block as stored, at `index`; check questions not yet answered. */
export function blockOf(block: SampleBlock, index: number): Block {
  switch (block.type) {
    case "Prose":
      return { index, content: { type: "Prose", text: proseText(block) } };
    case "Visual":
      return {
        index,
        content: {
          type: "Visual",
          spec: block.spec,
          description: block.description,
        },
      };
    case "Figure":
      return {
        index,
        content: {
          type: "Figure",
          svg: block.svg,
          description: block.description,
        },
      };
    case "Mermaid":
      return {
        index,
        content: {
          type: "Mermaid",
          source: block.source,
          description: block.description,
        },
      };
    case "Check":
      return {
        index,
        content: {
          type: "Check",
          questions: block.questions.map((question, number) => ({
            index: number,
            stem: question.stem,
            options: question.options,
            answer: null,
          })),
        },
      };
    case "Fallback":
      return {
        index,
        content: { type: "Fallback", description: block.description },
      };
  }
}

/**
 * The citations of a prose block: each phrase's first occurrence, counted
 * in Unicode characters as the core counts them.
 */
export function citationsOf(block: SampleBlock, index: number): Citation[] {
  if (block.type !== "Prose") return [];
  const text = proseText(block);
  return (block.cites ?? []).map((cite) => {
    const start = Array.from(text.slice(0, text.indexOf(cite.phrase))).length;
    return {
      block: index,
      start,
      end: start + Array.from(cite.phrase).length,
      source: cite.source,
    };
  });
}

export function versionOf(
  world: FakeWorld,
  sample: SampleSection,
  style: VersionStyle,
  createdAt: string,
): SectionVersion {
  const search = !world.searchRefused;
  return {
    id: crypto.randomUUID(),
    style,
    blocks: sample.blocks.map(blockOf),
    citations: search ? sample.blocks.flatMap(citationsOf) : [],
    sources: search ? sample.sources : [],
    writtenWithoutSearch: !search,
    model: world.models.current,
    createdAt,
  };
}

// -- Courses ------------------------------------------------------------------

export function sectionsOf(course: FakeCourse): FakeSection[] {
  const walk = (sections: FakeSection[]): FakeSection[] =>
    sections.flatMap((section) => [section, ...walk(section.deepDives ?? [])]);
  return walk(course.chapters.flatMap((chapter) => chapter.sections));
}

export function newSection(
  courseId: string,
  sample: SampleSection,
  parent: FakeSection | null,
): FakeSection {
  return {
    id: crypto.randomUUID(),
    courseId,
    kind: parent === null ? "BroadPass" : "DeepDive",
    sample,
    versions: [],
    currentVersionId: null,
    read: false,
    deepDives: null,
    parent,
  };
}

/** The sample's broad pass, every section unread. */
export function sampleChapters(courseId: string): FakeCourse["chapters"] {
  return sampleIndex.map((chapter) => ({
    id: crypto.randomUUID(),
    title: chapter.title,
    sections: chapter.sections.map((sample) =>
      newSection(courseId, sample, null),
    ),
  }));
}

export function newCourse(
  world: FakeWorld,
  topic: string,
  stage: CourseStage,
): FakeCourse {
  const now = iso(world);
  return {
    id: crypto.randomUUID(),
    topic,
    stage,
    chapters: [],
    placement: null,
    resumeAt: null,
    glossary: [],
    chats: [],
    createdAt: now,
    updatedAt: now,
    lastOpenedAt: now,
  };
}

export function current(section: FakeSection): SectionVersion | null {
  return (
    section.versions.find(
      (version) => version.id === section.currentVersionId,
    ) ?? null
  );
}

/** A complete version of the section, stored and made current. */
export function storeVersion(
  section: FakeSection,
  version: SectionVersion,
): void {
  section.versions.push(version);
  section.currentVersionId = version.id;
}

export function nodeOf(section: FakeSection): SectionNode {
  const deepDives = (section.deepDives ?? []).map(nodeOf);
  return {
    id: section.id,
    kind: section.kind,
    title: section.sample.title,
    summary: section.sample.summary,
    read: section.read,
    mark: deepDives.length > 0 ? "Branch" : section.read ? "Read" : "Unread",
    deepDives,
  };
}

export function chaptersOf(course: FakeCourse): ChapterNode[] {
  return course.chapters.map((chapter) => ({
    id: chapter.id,
    title: chapter.title,
    sections: chapter.sections.map(nodeOf),
  }));
}

export function courseView(course: FakeCourse): CourseView {
  return {
    id: course.id,
    topic: course.topic,
    stage: course.stage,
    chapters: chaptersOf(course),
    resumeAt: course.resumeAt ?? course.chapters[0]?.sections[0]?.id ?? null,
    glossaryCount: course.glossary.length,
    sideChatCount: course.chats.length,
    placement: course.placement?.state ?? null,
  };
}

export function summaryOf(course: FakeCourse): CourseSummary {
  const sections = sectionsOf(course);
  const resume = sections.find((section) => section.id === course.resumeAt);
  return {
    id: course.id,
    topic: course.topic,
    stage: course.stage,
    progress: {
      read: sections.filter((section) => section.read).length,
      total: sections.length,
    },
    resumeTitle: resume?.sample.title ?? null,
    lastOpenedAt: course.lastOpenedAt,
    createdAt: course.createdAt,
    updatedAt: course.updatedAt,
  };
}

/** The library, most recently opened first. */
export function librarySummaries(world: FakeWorld): CourseSummary[] {
  const opened = (course: FakeCourse) =>
    course.lastOpenedAt ?? course.createdAt;
  return [...world.courses]
    .sort((a, b) => opened(b).localeCompare(opened(a)))
    .map(summaryOf);
}

export function sectionView(
  section: FakeSection,
  version: SectionVersion,
): SectionView {
  return {
    id: section.id,
    courseId: section.courseId,
    title: section.sample.title,
    summary: section.sample.summary,
    version: structuredClone(version),
    versions: section.versions.map(({ id, style, createdAt }) => ({
      id,
      style,
      createdAt,
    })),
  };
}

export function chatSummary(chat: SideChat): SideChatSummary {
  const first = chat.messages.find((message) => message.role === "Candidate");
  return {
    id: chat.id,
    courseId: chat.courseId,
    anchor: chat.anchor,
    sectionTitle: chat.sectionTitle,
    opening: chat.anchor?.quote ?? first?.text ?? "",
    createdAt: chat.createdAt,
    updatedAt: chat.updatedAt,
  };
}

// -- The sample library -------------------------------------------------------

const readTitles = [
  "Layers by composition",
  "Lithosphere and asthenosphere",
  "Divergent boundaries",
  "Convergent boundaries",
];

/**
 * Plate tectonics part read, resuming at Convergent boundaries, which has its
 * deep dives proposed, a glossary entry and a side chat; and The French
 * Revolution, left in placement at its second question.
 */
export function addSampleLibrary(world: FakeWorld): void {
  world.courses.push(plateTectonics(world), frenchRevolution(world));
}

function plateTectonics(world: FakeWorld): FakeCourse {
  const course = newCourse(world, "Plate tectonics", "Ready");
  course.createdAt = iso(world, -12 * DAY);
  course.lastOpenedAt = iso(world, -HOUR);
  course.updatedAt = course.lastOpenedAt;
  course.chapters = sampleChapters(course.id);
  const sections = sectionsOf(course);
  for (const [day, title] of readTitles.entries()) {
    const section = sections.find((each) => each.sample.title === title)!;
    storeVersion(
      section,
      versionOf(
        world,
        section.sample,
        "Original",
        iso(world, (day - 10) * DAY),
      ),
    );
    section.read = true;
  }
  const convergent = sections.find(
    (section) => section.sample.title === "Convergent boundaries",
  )!;
  convergent.deepDives = (convergent.sample.deepDives ?? []).map((sample) =>
    newSection(course.id, sample, convergent),
  );
  course.resumeAt = convergent.id;

  const version = current(convergent)!;
  const anchor = (block: number, quote: string): Anchor => ({
    versionId: version.id,
    block,
    quote,
  });
  course.glossary.push({
    id: crypto.randomUUID(),
    courseId: course.id,
    sectionId: convergent.id,
    sectionTitle: convergent.sample.title,
    term: "asthenosphere",
    definition: definitions.asthenosphere!,
    anchor: anchor(0, "asthenosphere"),
    wikipedia: null,
    createdAt: iso(world, -2 * DAY),
  });
  const asked = iso(world, -2 * DAY + 5 * MINUTE);
  const reply = sideChatReply.blocks[0]!;
  course.chats.push({
    id: crypto.randomUUID(),
    courseId: course.id,
    anchor: anchor(
      3,
      "Its weight drags the rest of the plate behind it, a force called slab pull",
    ),
    sectionTitle: convergent.sample.title,
    messages: [
      {
        id: crypto.randomUUID(),
        role: "Candidate",
        text: sideChatQuestion,
        state: "Complete",
        citations: [],
        sources: [],
        createdAt: asked,
      },
      {
        id: crypto.randomUUID(),
        role: "Agent",
        text: reply.type === "Prose" ? proseText(reply) : "",
        state: "Complete",
        citations: citationsOf(reply, 0),
        sources: sideChatReply.sources,
        createdAt: asked,
      },
    ],
    followUps,
    createdAt: asked,
    updatedAt: asked,
  });
  return course;
}

function frenchRevolution(world: FakeWorld): FakeCourse {
  const course = newCourse(world, "The French Revolution", "Placement");
  course.createdAt = iso(world, -DAY - 2 * HOUR);
  course.lastOpenedAt = iso(world, -DAY);
  course.updatedAt = course.lastOpenedAt;
  const questionId = crypto.randomUUID();
  course.placement = {
    state: {
      step: {
        type: "Question",
        question: {
          id: questionId,
          number: 2,
          stem: frenchRevolutionQuestions[1]!.stem,
          options: frenchRevolutionQuestions[1]!.options,
        },
      },
      minQuestions: 4,
      maxQuestions: 6,
      error: null,
    },
    questions: frenchRevolutionQuestions,
    rating: "KnowTheBasics",
    goal: "Overview",
    answers: [{ type: "Option", index: 0 }],
    questionId,
  };
  return course;
}

// -- Usage --------------------------------------------------------------------

function level(used: number): UsageLevel {
  if (used >= 1) return "Limit";
  if (used >= 0.9) return "Critical";
  if (used >= 0.75) return "High";
  return "Normal";
}

/** A reading of the windows as they stand now. */
export function reading(world: FakeWorld): UsageReading {
  const [fiveHours, week, month] = world.usageShares;
  return {
    windows: [
      {
        name: "5h",
        lengthSeconds: 5 * 3600,
        used: fiveHours,
        resetsAt: iso(world, 2 * HOUR + 14 * MINUTE),
        level: level(fiveHours),
      },
      {
        name: "7d",
        lengthSeconds: 7 * 86_400,
        used: week,
        resetsAt: iso(world, 3 * DAY),
        level: level(week),
      },
      {
        name: "30d",
        lengthSeconds: 30 * 86_400,
        used: month,
        resetsAt: iso(world, 17 * DAY),
        level: level(month),
      },
    ],
    takenAt: iso(world),
    stale: null,
  };
}

/** The usage meter connected, with a reading taken now. */
export function connectUsage(world: FakeWorld): void {
  world.usage = { type: "Connected", reading: reading(world) };
}
