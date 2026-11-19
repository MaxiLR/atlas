import type { SampleSection } from "../sample-course";
import type { Scenario } from "../scenario";
import { newCourse, newSection, signIn } from "../world";

const orbitFigure = [
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 160" role="img">',
  '<ellipse cx="160" cy="80" rx="140" ry="60" fill="none" stroke="currentColor" stroke-width="1.5"/>',
  '<circle cx="110" cy="80" r="8" fill="currentColor"/>',
  "</svg>",
].join("");

/** A section with every kind of block a lesson streams. */
const thirdLaw: SampleSection = {
  title: "Kepler's third law",
  summary: "How long a planet takes to go round, from how far out it is.",
  sources: [],
  blocks: [
    {
      type: "Prose",
      chunks: [
        "A planet further from the Sun takes longer to go round it, ",
        "and not only because its path is longer: ",
        "it also moves more slowly.",
      ],
    },
    {
      type: "Prose",
      chunks: [
        "## The law\n\n",
        "The square of the period $T$ grows with the cube of the ",
        "semi-major axis $a$:\n\n",
        "$$\nT^2 = \\frac{4\\pi^2}{GM} a^3\n$$",
      ],
    },
    {
      type: "Figure",
      svg: orbitFigure,
      description: "An elliptical orbit with the Sun at one focus.",
    },
    {
      type: "Prose",
      chunks: [
        "You can check it for any planet with a few lines of code:\n\n",
        "```python\nimport math\n\n",
        "def period(a, mass):\n",
        "    return 2 * math.pi * math.sqrt(a ** 3 / (G * mass))\n```",
      ],
    },
    {
      type: "Mermaid",
      source: "flowchart LR\n  A[Distance] --> B[Period]",
      description: "Distance sets the period.",
    },
    {
      type: "Prose",
      chunks: [
        "Mars is about 1.52 times as far out as the Earth, ",
        "so its year lasts about $1.52^{3/2} \\approx 1.88$ Earth years.",
      ],
    },
  ],
};

export default {
  description:
    "Signed in; the course Kepler's laws, whose unread section streams prose, maths, code, a figure and a diagram.",
  setup(world) {
    signIn(world);
    const course = newCourse(world, "Kepler's laws", "Ready");
    course.chapters = [
      {
        id: crypto.randomUUID(),
        title: "Orbits",
        sections: [newSection(course.id, thirdLaw, null)],
      },
    ];
    world.courses.push(course);
  },
} satisfies Scenario;
