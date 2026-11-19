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
  type FakeCourse,
  type FakeSection,
  type FakeWorld,
} from "../world";

const DAY = 24 * 60 * 60_000;

/** The chapters of the long course, five broad-pass sections each. */
const chapters = [
  "Maps before writing",
  "Ptolemy and the grid",
  "Measuring the Earth from Eratosthenes' shadows to the triangulation surveys of the eighteenth century",
  "Medieval world maps",
  "Portolan charts",
  "The age of exploration",
  "Mercator's projection",
  "Atlases and their publishers",
  "National surveys",
  "Measuring longitude",
  "Thematic maps",
  "Maps at war",
  "Aerial photography",
  "Satellites and remote sensing",
  "Geographic information systems",
  "Maps on every phone",
];

const angles = ["sources", "instruments", "methods", "makers", "legacy"];

/** A long title for the first section, to be cut short in the rail. */
const longTitle =
  "How a clay tablet from Babylon placed the known world inside a ring of ocean, and what that shape tells us";

function sample(title: string): SampleSection {
  return {
    title,
    summary: `What ${title.toLowerCase()} covers.`,
    sources: [],
    blocks: [
      {
        type: "Prose",
        chunks: [`${title} is part of the history of mapmaking. `],
      },
    ],
  };
}

function read(world: FakeWorld, section: FakeSection, day: number) {
  storeVersion(
    section,
    versionOf(world, section.sample, "Original", iso(world, -day * DAY)),
  );
  section.read = true;
}

function under(
  course: FakeCourse,
  parent: FakeSection,
  titles: string[],
): FakeSection[] {
  parent.deepDives = titles.map((title) =>
    newSection(course.id, sample(title), parent),
  );
  return parent.deepDives;
}

/**
 * The history of mapmaking: sixteen chapters of five sections, eighty in the
 * broad pass, with a long chapter title and a long section title. The first
 * two chapters are read; Ptolemy and the grid: methods has three deep dives,
 * the first read and the second with three of its own.
 */
function mapmaking(world: FakeWorld): FakeCourse {
  const course = newCourse(world, "The history of mapmaking", "Ready");
  course.lastOpenedAt = iso(world, -2 * 60 * 60_000);
  course.chapters = chapters.map((title, number) => ({
    id: crypto.randomUUID(),
    title,
    sections: angles.map((angle, at) =>
      newSection(
        course.id,
        sample(number === 0 && at === 0 ? longTitle : `${title}: ${angle}`),
        null,
      ),
    ),
  }));
  const [first, second] = course.chapters;
  [...first!.sections, ...second!.sections].forEach((section, day) =>
    read(world, section, 20 - day),
  );
  const methods = second!.sections[2]!;
  const [latitude, longitude] = under(course, methods, [
    "Reading latitude from the stars",
    "Guessing longitude from travel times",
    "Copying the tables by hand",
  ]);
  read(world, latitude!, 5);
  under(course, longitude!, [
    "Itineraries and stages",
    "Marinus of Tyre",
    "Why Asia came out too wide",
  ]);
  course.resumeAt = second!.sections[4]!.id;
  return course;
}

export default {
  description:
    "The sample library and a long course: eighty sections in sixteen chapters, long titles and nested deep dives.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.courses.push(mapmaking(world));
  },
} satisfies Scenario;
