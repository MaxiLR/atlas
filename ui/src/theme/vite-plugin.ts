import type { HtmlTagDescriptor, Plugin } from "vite";

import { themeBootScript } from "./mode.ts";
import { themeStylesheet } from "./stylesheet.ts";

/**
 * Puts the generated theme into the page head: the tokens of both themes and
 * the script that picks one before the first paint. Nothing generated is
 * written to the repository.
 */
export function atlasTheme(): Plugin {
  const tags: HtmlTagDescriptor[] = [
    { tag: "script", children: themeBootScript, injectTo: "head-prepend" },
    { tag: "style", children: themeStylesheet(), injectTo: "head-prepend" },
  ];
  return {
    name: "atlas-theme",
    transformIndexHtml: () => tags,
  };
}
