import type { Oklch } from "culori";

export type ThemeInput = {
  /** The ground the theme is built on; it becomes the canvas. */
  base: string;
  /** The one saturated colour. */
  accent: string;
  /** 1 reproduces the reference; higher separates text further. */
  contrast: number;
};

export type TokenName =
  | "canvas"
  | "panel"
  | "raised"
  | "heading"
  | "body"
  | "muted"
  | "faint"
  | "edge"
  | "edgeStrong"
  | "wash"
  | "washStrong"
  | "accent"
  | "accentWash"
  | "primaryFill"
  | "primaryText"
  | "focus"
  | "green"
  | "amber"
  | "red";

export type Theme = {
  input: ThemeInput;
  scheme: "light" | "dark";
  tokens: Record<TokenName, Oklch>;
};

export const themeInputs = {
  dark: { base: "#08090a", accent: "#7a83e8", contrast: 1 },
  light: { base: "#fdfdfd", accent: "#5e6ad2", contrast: 1 },
} satisfies Record<string, ThemeInput>;

export function generateTheme(input: ThemeInput): Theme {
  throw new Error(`generateTheme is not implemented (${input.base})`);
}

export function deriveSurface(theme: Theme, background: Oklch | string): Theme {
  throw new Error(
    `deriveSurface is not implemented (${theme.scheme}, ${String(background)})`,
  );
}
