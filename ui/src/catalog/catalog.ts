/**
 * The catalog of learning visuals: the one definition the model's
 * instructions (`prompts/generated/catalog.prompt.md`), the core's validation
 * (`prompts/generated/catalog.schema.json`) and the registry all come from.
 *
 * `scripts/gen-catalog.ts` imports this file under Node, so it imports only
 * packages and relative `.ts` files.
 */
import { defineCatalog } from "@json-render/core";
import { schema } from "@json-render/react/schema";
import { z } from "zod";

/**
 * The version written in the generated prompt's front matter. Bump it
 * whenever the catalog changes, as for any prompt.
 */
export const CATALOG_VERSION = 1;

const caption = z
  .string()
  .min(1)
  .describe("One sentence saying what the visual shows.");

const labelled = z.strictObject({
  label: z.string().min(1).describe("A short name, a few words."),
  note: z.string().min(1).describe("One sentence."),
});

const conceptMap = z.strictObject({
  caption,
  nodes: z
    .array(
      z.strictObject({
        id: z.string().min(1).describe("Unique within the map."),
        label: z.string().min(1).describe("The concept, a few words."),
      }),
    )
    .min(2),
  links: z
    .array(
      z.strictObject({
        from: z.string().min(1).describe("The id of a node."),
        to: z.string().min(1).describe("The id of a node."),
        label: z.string().min(1).describe("How the two relate, a few words."),
        directed: z
          .boolean()
          .optional()
          .describe("True when the relation reads one way, from → to."),
      }),
    )
    .min(1),
});

const layeredFigure = z.strictObject({
  caption,
  layers: z
    .array(labelled)
    .min(2)
    .describe("Ordered from the top or outermost layer down."),
});

const timeline = z.strictObject({
  caption,
  events: z
    .array(
      z.strictObject({
        date: z.string().min(1).describe("A date, year or period, as written."),
        label: z.string().min(1).describe("What happened, a few words."),
        note: z.string().min(1).optional().describe("One sentence."),
      }),
    )
    .min(2)
    .describe("In date order, earliest first."),
});

const stepThrough = z.strictObject({
  caption,
  steps: z.array(labelled).min(2).describe("In order, first step first."),
});

const comparison = z.strictObject({
  caption,
  columns: z
    .array(z.string().min(1))
    .min(2)
    .describe("The things compared, one heading per column."),
  rows: z
    .array(
      z.strictObject({
        label: z.string().min(1).describe("The aspect compared in this row."),
        cells: z
          .array(z.string().min(1))
          .describe("One cell per column, in column order."),
      }),
    )
    .min(1),
});

const callout = z.strictObject({
  title: z.string().min(1).describe("The point, in one short sentence."),
  body: z.string().min(1).describe("One to three sentences."),
});

const mermaidDiagram = z.strictObject({
  source: z
    .string()
    .min(1)
    .describe(
      "Mermaid source, without init directives, click lines, links or HTML.",
    ),
  caption,
});

const figure = z.strictObject({
  svg: z
    .string()
    .min(1)
    .describe(
      "One static <svg> element: no script, no foreignObject, no event attributes, no external references; colours only currentColor or the theme's variables.",
    ),
  caption,
});

const checkQuestion = z.strictObject({
  rung: z.int().min(0).describe("The id of the ladder rung it tests."),
  stem: z.string().min(1).describe("The question."),
  options: z
    .array(
      z.strictObject({
        text: z.string().min(1),
        misconception: z
          .string()
          .min(1)
          .nullable()
          .describe(
            "For a wrong option, one sentence naming the misconception it represents; null for the right option.",
          ),
      }),
    )
    .length(4),
  key: z.int().min(0).max(3).describe("The right option, counted from 0."),
  explanation: z
    .string()
    .min(1)
    .describe("Why the right option is right, shown once answered."),
});

const citation = z.strictObject({
  number: z.int().min(1).describe("The source's number in the section."),
  title: z.string().min(1).describe("The source's title."),
  url: z
    .string()
    .regex(/^https?:\/\/\S+$/)
    .describe("The source's address, http or https."),
});

export const catalog = defineCatalog(schema, {
  components: {
    "concept-map": {
      props: conceptMap,
      description:
        "Concepts as nodes joined by labelled links, for how a handful of ideas relate.",
    },
    "layered-figure": {
      props: layeredFigure,
      description:
        "Stacked layers, each with a label and a note, for structures built in layers.",
    },
    timeline: {
      props: timeline,
      description: "Dated events in order, for how something developed.",
    },
    "step-through": {
      props: stepThrough,
      description:
        "Ordered steps, each with a label and a note, for a process or a derivation.",
    },
    comparison: {
      props: comparison,
      description:
        "A table comparing a few things (columns) on a few aspects (rows).",
    },
    callout: {
      props: callout,
      description: "One point set apart from the prose: a warning, a key idea.",
    },
    "mermaid-diagram": {
      props: mermaidDiagram,
      description:
        "A Mermaid flowchart, sequence or state diagram, for structure a catalog entry does not fit.",
    },
    figure: {
      props: figure,
      description:
        "A free-form static SVG drawing, for what only a drawing shows.",
    },
    "check-question": {
      props: checkQuestion,
      description:
        "A multiple-choice question with four options, one right, testing one rung.",
    },
    citation: {
      props: citation,
      description: "A reference to a numbered source of the section.",
    },
  },
  actions: {},
});

export type AtlasCatalog = typeof catalog;
export type CatalogName = keyof typeof catalog.data.components;
