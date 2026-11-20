// @vitest-environment node
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build } from "vite";
import { afterAll, describe, expect, it } from "vitest";

const scratch = await mkdtemp(join(tmpdir(), "atlas-bundle-"));
afterAll(() => rm(scratch, { recursive: true, force: true }));

async function bundledScript(mode: string): Promise<string> {
  const outDir = join(scratch, mode);
  await build({
    root: join(import.meta.dirname, "../../.."),
    mode,
    logLevel: "silent",
    build: { outDir, emptyOutDir: true },
  });
  const assets = join(outDir, "assets");
  const scripts = (await readdir(assets)).filter((file) =>
    file.endsWith(".js"),
  );
  const contents = await Promise.all(
    scripts.map((file) => readFile(join(assets, file), "utf8")),
  );
  return contents.join("\n");
}

describe("the fake core", () => {
  it("is left out of the production bundle", async () => {
    const script = await bundledScript("production");
    expect(script).toContain("core_request");
    expect(script).not.toContain("about as fast as fingernails grow");
    expect(script).not.toContain("plan-limit-no-reset");
  });

  it("is bundled in fake mode", async () => {
    const script = await bundledScript("fake");
    expect(script).toContain("about as fast as fingernails grow");
    expect(script).toContain("plan-limit-no-reset");
  });
});
