import type { SampleSection } from "../sample-course";
import type { Scenario } from "../scenario";
import {
  addSampleLibrary,
  iso,
  newCourse,
  newSection,
  signIn,
  storeVersion,
  versionOf,
  type FakeWorld,
} from "../world";

const HOUR = 60 * 60_000;
const DAY = 24 * HOUR;

/** A course on the shelf: its index size, how far it is read, when it was last opened. */
interface Shelved {
  topic: string;
  sections: number;
  read: number;
  /** The section it resumes at: the first unread one, or the last when all are read. */
  resumesAt: string;
  openedAgo: number;
}

const shelf: Shelved[] = [
  {
    topic: "Bayesian inference",
    sections: 22,
    read: 14,
    resumesAt: "Conjugate priors",
    openedAgo: DAY + 3 * HOUR,
  },
  {
    topic: "How central banks set interest rates",
    sections: 21,
    read: 21,
    resumesAt: "Quantitative easing and its exit",
    openedAgo: 7 * DAY,
  },
  {
    topic: "Ownership and borrowing in Rust",
    sections: 17,
    read: 3,
    resumesAt: "What a move does",
    openedAgo: 15 * DAY,
  },
  {
    topic:
      "The history and practice of cartography, from Ptolemy's Geography and the portolan charts to satellite geodesy and the projections behind modern web maps",
    sections: 30,
    read: 6,
    resumesAt: "Mercator and the problem of flattening a sphere onto a page",
    openedAgo: 21 * DAY,
  },
  {
    topic: "Linear algebra for machine learning",
    sections: 18,
    read: 9,
    resumesAt: "Eigenvalues and eigenvectors",
    openedAgo: 30 * DAY,
  },
  {
    topic: "Music theory for guitarists",
    sections: 15,
    read: 1,
    resumesAt: "Intervals on the fretboard",
    openedAgo: 41 * DAY,
  },
  {
    topic: "The immune system",
    sections: 20,
    read: 12,
    resumesAt: "How T cells learn what to ignore",
    openedAgo: 55 * DAY,
  },
  {
    topic: "Classical thermodynamics",
    sections: 16,
    read: 4,
    resumesAt: "Entropy as a state function",
    openedAgo: 80 * DAY,
  },
  {
    topic: "Roman law",
    sections: 14,
    read: 2,
    resumesAt: "The Twelve Tables",
    openedAgo: 140 * DAY,
  },
  {
    topic: "Japanese woodblock printing",
    sections: 12,
    read: 5,
    resumesAt: "Carving the key block",
    openedAgo: 400 * DAY,
  },
];

function sample(topic: string, title: string): SampleSection {
  return {
    title,
    summary: `A part of ${topic}.`,
    blocks: [
      {
        type: "Prose",
        chunks: [`${title} is one part of ${topic}, written for this course.`],
      },
    ],
    sources: [],
  };
}

function shelved(world: FakeWorld, entry: Shelved) {
  const course = newCourse(world, entry.topic, "Ready");
  course.createdAt = iso(world, -entry.openedAgo - 10 * DAY);
  course.lastOpenedAt = iso(world, -entry.openedAgo);
  course.updatedAt = course.lastOpenedAt;
  const resume = Math.min(entry.read, entry.sections - 1);
  const sections = Array.from({ length: entry.sections }, (_, index) =>
    newSection(
      course.id,
      sample(
        entry.topic,
        index === resume ? entry.resumesAt : `Part ${index + 1}`,
      ),
      null,
    ),
  );
  for (const section of sections.slice(0, entry.read)) {
    storeVersion(
      section,
      versionOf(world, section.sample, "Original", course.createdAt),
    );
    section.read = true;
  }
  course.chapters = [{ id: crypto.randomUUID(), title: entry.topic, sections }];
  course.resumeAt = sections[resume]!.id;
  return course;
}

export default {
  description:
    "Signed in with plan usage on; a long library, opened today, yesterday and on earlier dates, with a long topic and a course in placement.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.courses.push(...shelf.map((entry) => shelved(world, entry)));
  },
} satisfies Scenario;
