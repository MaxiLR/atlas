import type { Plugin } from "vite";

import { themeBootScript } from "./mode.ts";
import { themeStylesheet } from "./stylesheet.ts";

/** Where the page loads the tokens from, in development and in the build. */
const STYLESHEET_FILE = "assets/atlas-theme.css";

/**
 * Puts the generated theme into the page head: a stylesheet with the tokens of
 * both themes and the script that picks one before the first paint. The
 * tokens are a linked file, never an inline `<style>`: Tauri gives every
 * `<style>` element a nonce, and a nonce in `style-src` disables the
 * `'unsafe-inline'` that KaTeX and Mermaid output needs. Nothing generated is
 * written to the repository.
 */
export function atlasTheme(): Plugin {
  const stylesheet = themeStylesheet();
  let base = "/";
  return {
    name: "atlas-theme",
    configResolved: (config) => {
      base = config.base;
    },
    configureServer: (server) => {
      server.middlewares.use(
        `${base}${STYLESHEET_FILE}`,
        (_request, response) => {
          response.setHeader("Content-Type", "text/css");
          response.end(stylesheet);
        },
      );
    },
    generateBundle() {
      this.emitFile({
        type: "asset",
        fileName: STYLESHEET_FILE,
        source: stylesheet,
      });
    },
    transformIndexHtml: () => [
      { tag: "script", children: themeBootScript, injectTo: "head-prepend" },
      {
        tag: "link",
        attrs: { rel: "stylesheet", href: `${base}${STYLESHEET_FILE}` },
        injectTo: "head-prepend",
      },
    ],
  };
}
