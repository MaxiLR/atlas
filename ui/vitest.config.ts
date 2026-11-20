import { defineConfig, mergeConfig } from "vitest/config";

import viteConfig from "./vite.config.ts";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: "jsdom",
      include: ["src/**/*.test.{ts,tsx}"],
      setupFiles: ["src/test/setup.ts", "src/test/timeouts.ts"],
      // One jsdom worker per core starves the machine: the suite then takes
      // minutes and its tests time out. Half the cores runs it in seconds,
      // and a 30 s test timeout leaves room when other work slows it down.
      maxWorkers: "50%",
      testTimeout: 30_000,
    },
  }),
);
