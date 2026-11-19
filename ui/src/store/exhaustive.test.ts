// @vitest-environment node
import { join, relative, resolve } from "node:path";

import ts from "typescript";
import { describe, expect, it } from "vitest";

const ui = resolve(import.meta.dirname, "../..");
const generated = join(ui, "src/core/generated");

/**
 * The type errors in `src/store/` when the store is type-checked against the
 * generated protocol, with `variant` added to the union in `file` when given:
 * a protocol that grew a tag the store does not know.
 */
function storeErrors(extended?: { file: string; variant: string }): string[] {
  const config = ts.getParsedCommandLineOfConfigFile(
    join(ui, "tsconfig.app.json"),
    {},
    { ...ts.sys, onUnRecoverableConfigFileDiagnostic: () => {} },
  );
  if (config === undefined) throw new Error("tsconfig.app.json did not parse");
  const host = ts.createCompilerHost(config.options);
  const readSource = host.getSourceFile.bind(host);
  host.getSourceFile = (fileName, languageVersion, ...rest) => {
    if (
      extended !== undefined &&
      resolve(fileName) === join(generated, extended.file)
    ) {
      const text = ts.sys
        .readFile(fileName)!
        .replace(/;\s*$/, ` | ${extended.variant};\n`);
      return ts.createSourceFile(fileName, text, languageVersion);
    }
    return readSource(fileName, languageVersion, ...rest);
  };
  const program = ts.createProgram(
    [join(ui, "src/store/index.ts")],
    config.options,
    host,
  );
  return ts
    .getPreEmitDiagnostics(program)
    .filter((diagnostic) => diagnostic.file !== undefined)
    .map(
      (diagnostic) =>
        `${relative(ui, diagnostic.file!.fileName)}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n")}`,
    )
    .filter((error) => error.startsWith("src/store/"));
}

describe("the store's handling of the protocol", () => {
  it("type-checks against the protocol as generated", () => {
    expect(storeErrors()).toEqual([]);
  });

  it.each([
    ["CoreEvent.ts", "an event"],
    ["GenerationEvent.ts", "a generation event"],
    ["GenerationOutcome.ts", "a generation outcome"],
    ["CoreResponse.ts", "a response"],
  ])("fails type-checking for %s with an unknown tag (%s)", (file) => {
    const errors = storeErrors({ file, variant: '{ "type": "Unknown" }' });

    expect(errors).not.toEqual([]);
  });
});
