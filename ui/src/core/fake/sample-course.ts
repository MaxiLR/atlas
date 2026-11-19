/**
 * The fake core's sample course, Plate tectonics as the design drafts show
 * it: compact, with every kind of block, a visual that ends as its text
 * fallback, citations and sources, a check question, deep-dive proposals, a
 * glossary entry and a side chat.
 */
import type { Source } from "../generated/Source";
import type { VersionStyle } from "../generated/VersionStyle";

/** A check question with what the core keeps from the interface: its key. */
export interface KeyedCheck {
  stem: string;
  options: string[];
  key: number;
  explanation: string;
}

/**
 * A block as the fake writes it. Prose arrives in its chunks; a citation
 * covers the first occurrence of its phrase. A fallback is the visual that
 * failed validation twice: it opens as a visual and completes as text.
 */
export type SampleBlock =
  | {
      type: "Prose";
      chunks: string[];
      cites?: { phrase: string; source: number }[];
    }
  | { type: "Visual"; spec: unknown; description: string }
  | { type: "Figure"; svg: string; description: string }
  | { type: "Mermaid"; source: string; description: string }
  | { type: "Check"; questions: KeyedCheck[] }
  | { type: "Fallback"; description: string };

export interface SampleSection {
  title: string;
  summary: string;
  blocks: SampleBlock[];
  /** Numbered in order of first citation. */
  sources: Source[];
  /** What Go deeper proposes under it; generic ones when left out. */
  deepDives?: SampleSection[];
}

export interface SampleChapter {
  title: string;
  sections: SampleSection[];
}

/** A placement question as written: a stem and four options. */
export interface PlacementDraft {
  stem: string;
  options: string[];
}

const usgs: Source = {
  number: 1,
  url: "https://pubs.usgs.gov/gip/dynamic/understanding.html",
  title: "Understanding plate motions, U.S. Geological Survey",
};

const stern = (number: number): Source => ({
  number,
  url: "https://doi.org/10.1029/2001RG000108",
  title: "Subduction zones, R. J. Stern, Reviews of Geophysics",
});

function prose(...chunks: string[]): SampleBlock {
  return { type: "Prose", chunks };
}

const subductionFigure = [
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 140" role="img">',
  '<path d="M0 40 H150 L230 130" fill="none" stroke="currentColor" stroke-width="2"/>',
  '<path d="M150 30 H320" fill="none" stroke="currentColor" stroke-width="2"/>',
  '<path d="M180 30 L200 10 L220 30" fill="none" stroke="currentColor" stroke-width="1.5"/>',
  "</svg>",
].join("");

const convergent: SampleSection = {
  title: "Convergent boundaries",
  summary:
    "What happens where two plates meet head on, and why one of them almost always loses.",
  sources: [usgs, stern(2)],
  blocks: [
    {
      type: "Prose",
      chunks: [
        "Plates move toward each other at a few centimetres a year, about as fast as fingernails grow. ",
        "Where two of them converge, the surface cannot pile up forever. ",
        "One plate bends and slides beneath the other, a process called subduction, and sinks into the asthenosphere below.",
      ],
      cites: [{ phrase: "a few centimetres a year", source: 1 }],
    },
    prose(
      "Which plate goes under is decided by density. ",
      "Oceanic lithosphere is thinner but denser than continental lithosphere, and it grows colder and heavier as it ages. ",
      "When the two meet, the oceanic plate descends.",
    ),
    {
      type: "Figure",
      svg: subductionFigure,
      description:
        "An oceanic plate subducting beneath a continental plate. Not to scale.",
    },
    {
      type: "Prose",
      chunks: [
        "The sinking slab does more than disappear. ",
        "Its weight drags the rest of the plate behind it, a force called slab pull, ",
        "and the water it carries down lowers the melting point of the rock above, feeding a line of volcanoes parallel to the trench.",
      ],
      cites: [{ phrase: "a force called slab pull", source: 2 }],
    },
    {
      type: "Visual",
      spec: {
        root: "comparison",
        elements: {
          comparison: {
            type: "Comparison",
            props: {
              title: "Three kinds of convergence",
              columns: [
                "Ocean meets continent",
                "Ocean meets ocean",
                "Continent meets continent",
              ],
              rows: [
                {
                  label: "What sinks",
                  cells: [
                    "The oceanic plate",
                    "The older, colder plate",
                    "Neither, for long: both are too buoyant",
                  ],
                },
                {
                  label: "What forms",
                  cells: [
                    "A trench and a volcanic arc on land",
                    "A trench and an arc of volcanic islands",
                    "A wide, high mountain belt with few volcanoes",
                  ],
                },
                {
                  label: "Example",
                  cells: [
                    "Nazca plate beneath South America: the Andes",
                    "Pacific plate beneath the Mariana plate",
                    "India into Eurasia: the Himalaya",
                  ],
                },
              ],
            },
            children: [],
          },
        },
      },
      description: "The same collision, three outcomes.",
    },
    {
      type: "Mermaid",
      source: [
        "flowchart LR",
        "  A[Oceanic plate] --> B[Trench]",
        "  B --> C[Slab sinks]",
        "  C --> D[Water released]",
        "  D --> E[Mantle melts]",
        "  E --> F[Volcanic arc]",
      ].join("\n"),
      description: "From the trench to the volcanic arc, step by step.",
    },
    {
      type: "Fallback",
      description:
        "A timeline of the India–Eurasia collision: the ocean between them closed about 50 million years ago, and the Himalaya has been rising since.",
    },
    prose(
      "How slow is slow? ",
      "A plate moving at 7 cm a year covers $d = vt = 70$ km in a million years, ",
      "which is a short interval for a mountain belt.",
    ),
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
            "Ocean floor grows colder and denser as it ages, so the old plate is the heavier one and sinks beneath the younger, more buoyant plate.",
        },
      ],
    },
  ],
  deepDives: [
    {
      title: "Subduction, step by step",
      summary: "From the trench to the volcanic arc.",
      sources: [stern(1)],
      blocks: [
        {
          type: "Prose",
          chunks: [
            "Subduction starts where old ocean floor grows too dense to float. ",
            "It bends at the trench and sinks at an angle, dragging water-rich sediment with it. ",
            "At about 100 km depth the water escapes and melts the mantle above, feeding the volcanic arc.",
          ],
          cites: [{ phrase: "sinks at an angle", source: 1 }],
        },
      ],
    },
    {
      title: "Why the denser plate sinks",
      summary: "Density, age and temperature.",
      sources: [],
      blocks: [
        prose(
          "Density decides which plate gives way. ",
          "Oceanic rock is basalt, heavier than the granite-like rock of continents, ",
          "and the older it is, the colder and heavier it gets.",
        ),
      ],
    },
    {
      title: "Continental collision",
      summary: "When neither plate can sink.",
      sources: [],
      blocks: [
        prose(
          "When two continents meet, neither sinks for long: both are too buoyant. ",
          "The crust crumples and thickens instead, ",
          "raising the highest mountains on Earth, like the Himalaya.",
        ),
      ],
    },
  ],
};

/** The broad pass every course of the fake gets once placement is done. */
export const sampleIndex: SampleChapter[] = [
  {
    title: "The solid Earth",
    sections: [
      {
        title: "Layers by composition",
        summary: "Crust, mantle and core, by what they are made of.",
        sources: [],
        blocks: [
          prose(
            "Earth is layered by what its rock is made of. ",
            "A thin crust of light rock sits on a thick mantle of denser rock, around a core of iron and nickel. ",
            "The crust is to the Earth roughly what the skin is to an apple.",
          ),
        ],
      },
      {
        title: "Lithosphere and asthenosphere",
        summary: "The same layers, by how they behave.",
        sources: [],
        blocks: [
          prose(
            "Layered by how rock behaves, the picture changes. ",
            "The crust and the rigid top of the mantle form the lithosphere, broken into plates. ",
            "Beneath it the asthenosphere is solid but hot enough to flow, slowly, and the plates ride on it.",
          ),
        ],
      },
    ],
  },
  {
    title: "Plates and their boundaries",
    sections: [
      {
        title: "Divergent boundaries",
        summary: "Where plates pull apart and new ocean floor forms.",
        sources: [],
        blocks: [
          prose(
            "Where plates pull apart, the mantle below rises to fill the gap. ",
            "It melts as the pressure drops and cools into new ocean floor along a ridge. ",
            "The Mid-Atlantic Ridge widens the Atlantic by a few centimetres every year.",
          ),
        ],
      },
      convergent,
      {
        title: "Transform boundaries",
        summary: "Where plates slide past each other.",
        sources: [],
        blocks: [
          prose(
            "Where plates slide past each other, crust is neither made nor destroyed. ",
            "The boundary is a fault, like the San Andreas in California, ",
            "and it moves in jerks that we feel as earthquakes.",
          ),
        ],
      },
    ],
  },
  {
    title: "What moves the plates",
    sections: [
      {
        title: "Mantle convection",
        summary: "The slow churning beneath the plates.",
        sources: [],
        blocks: [
          prose(
            "The mantle is solid, yet over millions of years it flows. ",
            "Rock heated near the core rises and cooler rock sinks, ",
            "a slow churning like soup on a stove.",
          ),
        ],
      },
      {
        title: "Slab pull and ridge push",
        summary: "The two forces that act on the plates themselves.",
        sources: [],
        blocks: [
          prose(
            "Two forces act on the plates themselves. ",
            "Ridge push nudges a plate away from the high, warm ridge where it formed, ",
            "and slab pull, the weight of a sinking edge, drags it towards the trench, and is the larger of the two.",
          ),
        ],
      },
    ],
  },
];

/** Three deep dives for a section the sample has none written for. */
export function genericDeepDives(title: string): SampleSection[] {
  const topic = title.toLowerCase();
  return [
    ["in the field", "Where to see it on the map today."],
    ["measured", "How we know, and how precisely."],
    ["open questions", "What is still debated."],
  ].map(([angle, summary]) => ({
    title: `${title}: ${angle}`,
    summary,
    sources: [],
    blocks: [
      prose(
        `This deep dive looks closer at ${topic}. `,
        `It starts from what the section above it said, `,
        `and adds what the broad pass left out.`,
      ),
    ],
  }));
}

const styleOpenings: Record<Exclude<VersionStyle, "Original">, string> = {
  Simpler: "Put simply: ",
  MoreTechnical: "In more technical terms: ",
  MoreExamples: "Take an example first. ",
};

/** The section written another way: the same blocks, opened differently. */
export function styled(
  section: SampleSection,
  style: VersionStyle,
): SampleSection {
  if (style === "Original") return section;
  const first = section.blocks.findIndex((block) => block.type === "Prose");
  return {
    ...section,
    blocks: section.blocks.map((block, index) =>
      index === first && block.type === "Prose"
        ? { ...block, chunks: [styleOpenings[style], ...block.chunks] }
        : block,
    ),
  };
}

/** The questions placement asks for any new course. */
export const plateTectonicsQuestions: PlacementDraft[] = [
  {
    stem: "An oceanic plate and a continental plate are moving toward each other. What happens where they meet?",
    options: [
      "The oceanic plate bends and sinks beneath the continental plate.",
      "The continental plate sinks beneath the oceanic plate, because it is the thicker and heavier of the two.",
      "Both plates crumple upward into a mountain range, and neither one sinks.",
      "The two plates lock together and stop moving.",
    ],
  },
  {
    stem: "Two plates pull apart in the middle of an ocean. What forms between them?",
    options: [
      "A ridge of new ocean floor, built from rising mantle rock.",
      "A deep trench, as the floor between them sinks.",
      "A mountain belt, as the edges fold upward.",
      "Nothing: the gap stays open and fills with seawater.",
    ],
  },
  {
    stem: "A slab of old ocean floor sinks into the mantle. What happens to the plate it is attached to?",
    options: [
      "It is pulled toward the trench by the sinking slab's weight.",
      "It stops, because the slab anchors it in place.",
      "It moves away from the trench, pushed by the slab.",
      "It breaks off, and only the slab keeps moving.",
    ],
  },
  {
    stem: "You heat a pot of thick soup from below. What does the soup do, and what does it suggest about the mantle?",
    options: [
      "It rises where heated and sinks where cooler, as mantle rock does over millions of years.",
      "It stays still until it boils, as the mantle stays still until an eruption.",
      "It flows only at the surface, as only the crust moves.",
      "It cools from below, as the mantle cools from the core outward.",
    ],
  },
];

/** The questions of the course the default library has left in placement. */
export const frenchRevolutionQuestions: PlacementDraft[] = [
  {
    stem: "The Estates-General meets in May 1789 and votes by order. What does the Third Estate do when it is outvoted?",
    options: [
      "It declares itself the National Assembly and claims to speak for the nation.",
      "It withdraws and asks the king to dissolve the Estates-General.",
      "It joins the clergy in one chamber under noble leadership.",
      "It accepts the vote and goes home.",
    ],
  },
  {
    stem: "Paris crowds take the Bastille in July 1789. What happens in the countryside in the weeks after?",
    options: [
      "Peasants attack manor houses and burn the records of feudal dues.",
      "Peasants march on Paris to restore the king's authority.",
      "The countryside stays quiet until the next harvest.",
      "Nobles arm the peasants against the city.",
    ],
  },
  {
    stem: "The king tries to flee Paris in June 1791. What does his arrest at Varennes change?",
    options: [
      "Trust in a constitutional monarchy collapses, and calls for a republic grow.",
      "The Assembly strengthens his powers to calm the crowds.",
      "Foreign powers recall their armies.",
      "Nothing: the flight is kept secret.",
    ],
  },
  {
    stem: "France is at war and food prices soar in 1793. What does the Committee of Public Safety do?",
    options: [
      "It rules by emergency decree and executes suspected enemies: the Terror.",
      "It makes peace with Austria to bring prices down.",
      "It hands power back to the king.",
      "It dissolves itself and calls elections.",
    ],
  },
];

/** Definitions the fake writes for terms it knows, by lower-case term. */
export const definitions: Record<string, string> = {
  asthenosphere:
    "The layer of the mantle directly beneath the plates. It is solid rock, but hot enough to flow a few centimetres a year, which is what lets the rigid plates above slide over it. Here it is where the sinking plate goes.",
  subduction:
    "The sinking of one plate beneath another where two plates converge. The denser plate bends at a trench and descends into the mantle, carrying water that feeds volcanoes above it.",
};

/** The Wikipedia articles the fake finds, by lower-case term. */
export const articles: Record<
  string,
  { title: string; extract: string; url: string }
> = {
  asthenosphere: {
    title: "Asthenosphere",
    extract:
      "The asthenosphere is the mechanically weak and ductile region of the upper mantle of Earth. It lies below the lithosphere, between about 80 and 200 km below the surface, and extends as deep as 700 km.",
    url: "https://en.wikipedia.org/wiki/Asthenosphere",
  },
  subduction: {
    title: "Subduction",
    extract:
      "Subduction is a geological process in which the oceanic lithosphere and some continental lithosphere is recycled into the Earth's mantle at convergent boundaries between tectonic plates.",
    url: "https://en.wikipedia.org/wiki/Subduction",
  },
};

/** How the side chat of the sample course was answered, and every reply since. */
export const sideChatReply: SampleSection = {
  title: "Reply",
  summary: "",
  sources: [
    {
      number: 1,
      url: "https://doi.org/10.1111/j.1365-246X.1975.tb00631.x",
      title:
        "On the relative importance of the driving forces of plate motion, Forsyth and Uyeda",
    },
    { ...usgs, number: 2 },
  ],
  blocks: [
    {
      type: "Prose",
      chunks: [
        "For plates with a subducting edge, slab pull is the larger force. ",
        "Plates attached to long subduction zones, like the Pacific, move several times faster than plates with almost none. ",
        "Convection is the bigger picture the slab belongs to, not a rival: the sinking slab is the cold, downward limb of the mantle's circulation.",
      ],
      cites: [
        { phrase: "slab pull is the larger force", source: 1 },
        { phrase: "the bigger picture the slab belongs to", source: 2 },
      ],
    },
  ],
};

export const sideChatQuestion =
  "Is slab pull the main thing that moves plates, or is it convection in the mantle?";

export const followUps = [
  "What is ridge push, and how much does it add?",
  "How do we know which plates move fastest?",
  "What happens to a slab once it has sunk?",
];
