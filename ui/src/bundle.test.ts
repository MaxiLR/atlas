// @vitest-environment node
import { resolve } from "node:path";
import { build, type Rolldown } from "vite";
import { expect, test } from "vitest";

const ENTRY = "\0bundle-test-entry";

/** Builds an entry importing `source` with the app's own Vite configuration. */
async function bundle(source: string): Promise<Rolldown.OutputBundle> {
  const ui = resolve(import.meta.dirname, "..");
  const result = await build({
    configFile: resolve(ui, "vite.config.ts"),
    root: ui,
    logLevel: "silent",
    plugins: [
      {
        name: "bundle-test-entry",
        resolveId: (id) => (id === "bundle-test-entry" ? ENTRY : undefined),
        load: (id) => (id === ENTRY ? source : undefined),
      },
    ],
    build: {
      write: false,
      rollupOptions: { input: "bundle-test-entry" },
    },
  });
  const outputs = Array.isArray(result) ? result : [result];
  return Object.fromEntries(
    outputs.flatMap((output) => {
      if (!("output" in output)) throw new Error("expected a finished build");
      return output.output.map((file) => [file.fileName, file]);
    }),
  );
}

test("KaTeX's fonts ship as the app's own files, which the webview's font-src allows", async () => {
  const files = Object.values(await bundle('import "katex/dist/katex.css";'));

  const css = files
    .filter((file) => file.fileName.endsWith(".css"))
    .map((file) => (file.type === "asset" ? String(file.source) : ""))
    .join("\n");
  expect(css).toContain("KaTeX_Size3");
  expect(css).not.toMatch(/data:(font|application\/(x-)?font)/);
  // KaTeX_Size3-Regular.woff2 is under Vite's default inline limit.
  expect(
    files.some((file) =>
      /KaTeX_Size3-Regular-[\w-]+\.woff2$/.test(file.fileName),
    ),
  ).toBe(true);
}, 60_000);
