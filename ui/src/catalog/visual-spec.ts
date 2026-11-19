/**
 * What a catalog visual is: a json-render spec holding exactly one element of
 * the catalog, with plain props and no children, state, actions or
 * visibility. Checked here for the interface and exported as the JSON schema
 * the core validates with; both are built from the catalog's props.
 */
import type { Spec } from "@json-render/core";
import { z } from "zod";

import { catalog, type CatalogName } from "./catalog.ts";

const NAMES = catalog.componentNames as CatalogName[];

function isCatalogName(name: string): name is CatalogName {
  return (NAMES as string[]).includes(name);
}

const envelope = z.strictObject({
  root: z.string().min(1),
  elements: z.record(
    z.string(),
    z.strictObject({
      type: z.string(),
      props: z.record(z.string(), z.unknown()),
      children: z.array(z.string()).max(0).optional(),
    }),
  ),
});

function issues(error: z.ZodError, at: string): string[] {
  return error.issues.map((issue) => {
    const path = [at, ...issue.path.map(String)].filter(Boolean).join("/");
    return path ? `at /${path}: ${issue.message}` : issue.message;
  });
}

export type VisualSpecCheck =
  { ok: true; spec: Spec } | { ok: false; reasons: string[] };

/** Whether `spec` is a catalog visual, and if not, why. */
export function checkVisualSpec(spec: unknown): VisualSpecCheck {
  const shape = envelope.safeParse(spec);
  if (!shape.success) return { ok: false, reasons: issues(shape.error, "") };

  const { root, elements } = shape.data;
  const keys = Object.keys(elements);
  const reasons: string[] = [];
  if (keys.length !== 1) {
    reasons.push(`a visual holds exactly one element, not ${keys.length}`);
  }
  if (!keys.includes(root)) {
    reasons.push(`the root "${root}" names no element`);
  }
  for (const [key, element] of Object.entries(elements)) {
    if (!isCatalogName(element.type)) {
      reasons.push(
        `at /elements/${key}/type: unknown component "${element.type}"; the catalog has ${NAMES.join(", ")}`,
      );
      continue;
    }
    const props = catalog.data.components[element.type].props.safeParse(
      element.props,
    );
    if (!props.success) {
      reasons.push(...issues(props.error, `elements/${key}/props`));
    }
  }
  return reasons.length > 0
    ? { ok: false, reasons }
    : { ok: true, spec: shape.data };
}

type JsonSchema = Record<string, unknown>;

/** The JSON schema of one entry's props, to sit inside the visual's schema. */
export function propsJsonSchema(name: CatalogName): JsonSchema {
  const schema: JsonSchema = z.toJSONSchema(
    catalog.data.components[name].props,
    { target: "draft-2020-12" },
  );
  delete schema.$schema;
  return schema;
}

/**
 * The JSON schema of a catalog visual. It cannot say that `root` names the
 * one element; the core checks that beside the schema.
 */
export function visualSpecJsonSchema(): JsonSchema {
  return {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    title: "Catalog visual",
    description:
      "A json-render spec holding exactly one element of the catalog; root names it.",
    type: "object",
    required: ["root", "elements"],
    additionalProperties: false,
    properties: {
      root: { type: "string", minLength: 1 },
      elements: {
        type: "object",
        minProperties: 1,
        maxProperties: 1,
        additionalProperties: { $ref: "#/$defs/element" },
      },
    },
    $defs: {
      element: {
        type: "object",
        required: ["type", "props"],
        additionalProperties: false,
        properties: {
          type: { enum: NAMES },
          props: { type: "object" },
          children: { type: "array", maxItems: 0, items: { type: "string" } },
        },
        allOf: NAMES.map((name) => ({
          if: { properties: { type: { const: name } }, required: ["type"] },
          then: { properties: { props: { $ref: `#/$defs/props/${name}` } } },
        })),
      },
      props: Object.fromEntries(
        NAMES.map((name) => [name, propsJsonSchema(name)]),
      ),
    },
  };
}
