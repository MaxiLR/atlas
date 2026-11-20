// Writes the catalog's prompt and JSON schema, generated from the TypeScript
// catalog, into prompts/generated/ (or the folder given as the argument). The
// core embeds both files. Run by `npm run build` before the bundle.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { format } from "prettier";

import {
  catalog,
  CATALOG_VERSION,
  type CatalogName,
} from "../src/catalog/catalog.ts";
import {
  propsJsonSchema,
  visualSpecJsonSchema,
} from "../src/catalog/visual-spec.ts";

type JsonSchema = {
  type?: string;
  description?: string;
  properties?: Record<string, JsonSchema>;
  required?: string[];
  items?: JsonSchema;
  anyOf?: JsonSchema[];
  minItems?: number;
  maxItems?: number;
  minimum?: number;
  maximum?: number;
};

const ui = dirname(dirname(fileURLToPath(import.meta.url)));
const NAMES = catalog.componentNames as CatalogName[];

/** How many items a list holds, in words, before the items' noun. */
function count(min: number | undefined, max: number | undefined): string {
  if (min !== undefined && min === max) return `exactly ${min}`;
  if (min !== undefined && min > 0) return `${min} or more`;
  return "any number of";
}

/** A prop's type in words, as the prompt lists it. */
function typeOf(schema: JsonSchema): string {
  if (schema.anyOf) return schema.anyOf.map(typeOf).join(" or ");
  switch (schema.type) {
    case "string":
    case "boolean":
    case "null":
      return schema.type;
    case "integer": {
      const { minimum: min, maximum: max } = schema;
      if (min !== undefined && max !== undefined && max < 2 ** 31) {
        return `integer from ${min} to ${max}`;
      }
      return min !== undefined ? `integer, at least ${min}` : "integer";
    }
    case "array": {
      const items = schema.items ?? {};
      const noun = items.type === "object" ? "objects" : `${typeOf(items)}s`;
      return `list of ${count(schema.minItems, schema.maxItems)} ${noun}`;
    }
    case "object":
      return "object";
    default:
      throw new Error(`the prompt cannot describe ${JSON.stringify(schema)}`);
  }
}

/** One bullet per property, nested for objects in lists. */
function describeProps(schema: JsonSchema, depth = 0): string[] {
  const indent = "  ".repeat(depth);
  const required = new Set(schema.required ?? []);
  return Object.entries(schema.properties ?? {}).flatMap(([name, prop]) => {
    const optional = required.has(name) ? "" : ", optional";
    const description = prop.description ? `: ${prop.description}` : "";
    const line = `${indent}- \`${name}\` (${typeOf(prop)}${optional})${description}`;
    const nested =
      prop.type === "array" && prop.items?.type === "object"
        ? describeProps(prop.items, depth + 1)
        : [];
    return [line, ...nested];
  });
}

function entry(name: CatalogName): string {
  const { description } = catalog.data.components[name];
  return [
    `### \`${name}\``,
    "",
    description,
    "",
    ...describeProps(propsJsonSchema(name) as JsonSchema),
  ].join("\n");
}

function prompt(): string {
  return [
    "---",
    `version: ${CATALOG_VERSION}`,
    "---",
    "",
    "A catalog visual is one JSON object: a json-render spec holding exactly one element.",
    "",
    "```json",
    '{"root":"<key>","elements":{"<key>":{"type":"<entry>","props":{}}}}',
    "```",
    "",
    "- `root` is the key of the one element in `elements`.",
    "- `type` is one of the entries below; `props` holds that entry's props and no others. A prop is required unless marked optional.",
    "- Values are plain JSON. Do not use expressions such as `$state` or `$cond`, and do not add child elements or the fields `visible`, `on`, `repeat`, `slots` or `watch`.",
    "",
    "## Entries",
    "",
    NAMES.map(entry).join("\n\n"),
    "",
  ].join("\n");
}

/** The generated files by name, formatted as the repository's Prettier does. */
export async function generateCatalog(): Promise<Record<string, string>> {
  const files: Record<string, string> = {
    "catalog.prompt.md": prompt(),
    "catalog.schema.json": JSON.stringify(visualSpecJsonSchema()),
  };
  const formatted: Record<string, string> = {};
  for (const [name, text] of Object.entries(files)) {
    formatted[name] = await format(text, { filepath: name });
  }
  return formatted;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const out = resolve(process.argv[2] ?? join(ui, "../prompts/generated"));
  mkdirSync(out, { recursive: true });
  for (const [name, text] of Object.entries(await generateCatalog())) {
    writeFileSync(join(out, name), text);
  }
}
