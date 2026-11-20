import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

import { atlasTheme } from "./src/theme/vite-plugin.ts";

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss(), atlasTheme()],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
  build: {
    // The webview's font-src allows only the app's own files, so a small font
    // (KaTeX_Size3) is never inlined as a data: URL.
    assetsInlineLimit: (file) =>
      /\.(woff2?|ttf|otf|eot)$/i.test(file) ? false : undefined,
  },
});
