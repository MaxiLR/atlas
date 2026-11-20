import type { Oklch } from "culori";

import {
  deriveSurface,
  generateTheme,
  themeInputs,
  type Scheme,
  type Theme,
} from "./generate.ts";

const round = (value: number, places: number) => Number(value.toFixed(places));

/** `edgeStrong` → `edge-strong`. */
const cssName = (token: string) =>
  token.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);

export function formatOklch(color: Oklch): string {
  const hue = color.h === undefined || color.c < 0.0001 ? 0 : round(color.h, 2);
  const alpha =
    color.alpha === undefined || color.alpha >= 1
      ? ""
      : ` / ${round(color.alpha, 3)}`;
  return `oklch(${round(color.l, 4)} ${round(color.c, 4)} ${hue}${alpha})`;
}

function declarations(theme: Theme, prefix: string) {
  return Object.entries(theme.tokens).map(
    ([name, color]) => `  ${prefix}${cssName(name)}: ${formatOklch(color)};`,
  );
}

/**
 * The generated part of the theme: both themes' tokens as `--atlas-*`
 * variables under `[data-theme]`, a raised surface's re-derived tokens under
 * `[data-surface="raised"]`, and the ground each scope paints. Built at build
 * time by the atlas-theme Vite plugin; never committed.
 */
export function themeStylesheet(): string {
  const schemes = Object.keys(themeInputs) as Scheme[];
  const themes = schemes.map((scheme) => {
    const theme = generateTheme(themeInputs[scheme]);
    return {
      theme,
      raised: deriveSurface(theme, theme.tokens.raised),
    };
  });

  const scopes = themes.map(({ theme, raised }) =>
    [
      `[data-theme="${theme.scheme}"] {`,
      `  color-scheme: ${theme.scheme};`,
      ...declarations(theme, "--atlas-"),
      ...declarations(raised, "--atlas-raised-surface-"),
      "}",
    ].join("\n"),
  );

  // A raised surface takes the derived values of the nearest theme scope.
  const raisedSurface = [
    `[data-surface="raised"] {`,
    ...Object.keys(themes[0].theme.tokens).map(
      (name) =>
        `  --atlas-${cssName(name)}: var(--atlas-raised-surface-${cssName(name)});`,
    ),
    "}",
  ].join("\n");

  return [
    // Tailwind's layer order, declared before any layer is used.
    "@layer theme, base, components, utilities;",
    ...scopes,
    raisedSurface,
    `@layer base {
  [data-theme],
  [data-surface] {
    background-color: var(--atlas-canvas);
    color: var(--atlas-body);
  }
}`,
  ].join("\n");
}
