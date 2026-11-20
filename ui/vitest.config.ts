import { defineConfig, mergeConfig } from "vitest/config";

import viteConfig from "./vite.config.ts";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: "jsdom",
      include: ["src/**/*.test.{ts,tsx}"],
      setupFiles: ["src/test/setup.ts"],
      // One jsdom worker per core starves the machine: the suite then takes
      // minutes and its tests time out. Half the cores runs it in seconds.
      maxWorkers: "50%",
    },
  }),
);
