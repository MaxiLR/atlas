// @vitest-environment node
import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

const UI = resolve(import.meta.dirname, "../..");
const GENERATOR = resolve(UI, "scripts/gen-catalog.ts");
const COMMITTED = resolve(UI, "../prompts/generated");
const FILES = ["catalog.prompt.md", "catalog.schema.json"];

const folders: string[] = [];

afterEach(() => {
  for (const folder of folders.splice(0)) {
    rmSync(folder, { recursive: true, force: true });
  }
});

/** Runs the generator into a fresh folder and returns what it wrote. */
function generate(): Record<string, string> {
  const out = mkdtempSync(join(tmpdir(), "atlas-catalog-"));
  folders.push(out);
  execFileSync(process.execPath, [GENERATOR, out], { cwd: UI });
  return Object.fromEntries(
    readdirSync(out)
      .sort()
      .map((file) => [file, readFileSync(join(out, file), "utf8")]),
  );
}

describe("the catalog generator", () => {
  it("writes the prompt and the schema", () => {
    const files = generate();
    expect(Object.keys(files)).toEqual(FILES);
    expect(files["catalog.prompt.md"]).toMatch(/^---\nversion: \d+\n---\n/);
    expect(JSON.parse(files["catalog.schema.json"])).toMatchObject({
      type: "object",
    });
  });

  it("gives identical output when run twice", () => {
    expect(generate()).toEqual(generate());
  });

  it("matches the committed files", () => {
    const files = generate();
    for (const file of FILES) {
      expect(
        readFileSync(join(COMMITTED, file), "utf8"),
        `prompts/generated/${file} is stale: run npm --prefix ui run build`,
      ).toBe(files[file]);
    }
  });
});
