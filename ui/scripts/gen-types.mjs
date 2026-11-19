// Regenerates src/core/generated/ from the core's Rust types (ts-rs).
// The export directory is set in the repository's .cargo/config.toml.
import { spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ui = dirname(dirname(fileURLToPath(import.meta.url)));
const root = dirname(ui);

rmSync(join(ui, "src/core/generated"), { recursive: true, force: true });

const result = spawnSync(
  "cargo",
  ["test", "--locked", "--package", "atlas-core", "--lib", "export_bindings"],
  {
    cwd: root,
    stdio: "inherit",
    env: {
      ...process.env,
      PATH: [join(homedir(), ".cargo", "bin"), process.env.PATH].join(
        delimiter,
      ),
    },
  },
);
process.exit(result.status ?? 1);
