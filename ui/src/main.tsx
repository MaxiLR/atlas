import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./index.css";
import { App } from "./App";
import type { CoreClient } from "./core/client";
import { CoreProvider } from "./core/context";
import { ThemeProvider } from "./components/theme-provider";

async function createClient(): Promise<CoreClient> {
  // Vite replaces MODE at build time, so the fake core is left out of every
  // bundle that is not built in `fake` mode.
  if (import.meta.env.MODE === "fake") {
    const { createFakeCore } = await import("./core/fake");
    return createFakeCore();
  }
  const { createTauriCore } = await import("./core/tauri");
  return createTauriCore();
}

const client = await createClient();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider>
      <CoreProvider client={client}>
        <App />
      </CoreProvider>
    </ThemeProvider>
  </StrictMode>,
);
