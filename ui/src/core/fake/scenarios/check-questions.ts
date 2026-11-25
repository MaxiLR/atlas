import type { SampleSection } from "../sample-course";
import type { Scenario } from "../scenario";
import {
  iso,
  newCourse,
  newSection,
  signIn,
  storeVersion,
  versionOf,
} from "../world";

/** A read section that ends in a check block of two questions. */
const convergent: SampleSection = {
  title: "Convergent boundaries",
  summary:
    "What happens where two plates meet head on, and why one of them almost always loses.",
  sources: [],
  blocks: [
    {
      type: "Prose",
      chunks: [
        "The numbers are small until you give them geological time. ",
        "A plate moving at 7 cm a year covers 70 km in a million years, which is a short interval for a mountain belt. ",
        "Where both plates carry continents, neither sinks for long: the crust crumples, thickens and rises instead.",
      ],
    },
    {
      type: "Check",
      questions: [
        {
          stem: "A plate carrying young, warm ocean floor meets a plate carrying old, cold ocean floor. What happens next?",
          options: [
            "The old plate sinks beneath the young one.",
            "The young plate sinks, because it is hotter and softer.",
            "Neither sinks; they build a mountain belt like the Himalaya.",
            "They slide past each other along a fault.",
          ],
          key: 0,
          explanation:
            "Ocean floor grows colder and denser as it ages, so the older plate is the heavier one and descends.",
        },
        {
          stem: "Why does a collision between two continents build mountains instead of a trench?",
          options: [
            "Continental crust is too buoyant to sink far, so it crumples and thickens.",
            "Continental plates move faster, so they pile up before either one can sink.",
            "Mountains form only where melt from a sinking ocean plate rises through a continent.",
            "The mantle beneath a continent is too cold for a plate to descend into it.",
          ],
          key: 0,
          explanation:
            "Continental crust is thick and light. It resists being pushed down, so the shortening goes up instead, into a wide, high belt.",
        },
      ],
    },
    {
      type: "Prose",
      chunks: ["Trenches and mountains are the sum of millions of years."],
    },
  ],
};

export default {
  description:
    "Signed in; the course Plate boundaries, whose read section Convergent boundaries ends in a check block of two questions.",
  setup(world) {
    signIn(world);
    const course = newCourse(world, "Plate boundaries", "Ready");
    const section = newSection(course.id, convergent, null);
    storeVersion(section, versionOf(world, convergent, "Original", iso(world)));
    section.read = true;
    course.chapters = [
      { id: crypto.randomUUID(), title: "Boundaries", sections: [section] },
    ];
    world.courses.push(course);
  },
} satisfies Scenario;
