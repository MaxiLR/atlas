import { createServer } from "node:net";

import { defineConfig, devices } from "@playwright/test";

/** A port the OS reports free right now. */
async function freePort(): Promise<number> {
  const server = createServer();
  const { promise, resolve, reject } = Promise.withResolvers<number>();
  server.once("error", reject);
  server.listen(0, "127.0.0.1", () => {
    const address = server.address();
    server.close(() =>
      typeof address === "object" && address !== null
        ? resolve(address.port)
        : reject(new Error("no port")),
    );
  });
  return promise;
}

// Workers load this file again; they inherit the port chosen by the runner.
process.env.ATLAS_E2E_PORT ??= String(await freePort());
const port = Number(process.env.ATLAS_E2E_PORT);

const widths = [720, 1024, 1440];
const schemes = ["light", "dark"] as const;

export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  // Each worker drives its own Chromium against the one Vite server. On a
  // machine busy with other work, half the cores (the default) and the
  // default timeouts let the long keyboard journeys and the waits on
  // streamed text run out; a quarter of the cores and doubled timeouts keep
  // them inside.
  workers: "25%",
  timeout: 60_000,
  expect: { timeout: 10_000 },
  forbidOnly: true,
  reporter: "list",
  use: {
    ...devices["Desktop Chrome"],
    baseURL: `http://127.0.0.1:${port}`,
    headless: true,
    launchOptions: { args: ["--no-sandbox"] },
    trace: "retain-on-failure",
  },
  projects: [
    ...widths.flatMap((width) =>
      schemes.map((colorScheme) => ({
        name: `${width}-${colorScheme}`,
        testMatch: /e2e\/matrix\/.+\.spec\.ts$/,
        use: { viewport: { width, height: 900 }, colorScheme },
      })),
    ),
    {
      name: "default",
      testMatch: /e2e\/[^/]+\.spec\.ts$/,
      use: {
        viewport: { width: 1440, height: 900 },
        colorScheme: "dark",
      },
    },
  ],
  webServer: {
    command: `vite --mode fake --host 127.0.0.1 --port ${port} --strictPort`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
