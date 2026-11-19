import { differenceEuclidean, wcagContrast, type Color } from "culori";
import { describe, expect, it } from "vitest";

import {
  deriveSurface,
  generateTheme,
  themeInputs,
  type Theme,
} from "./generate";

/** Distance in OKLab, the Cartesian form of OKLCH; 0.02 is about one JND. */
const distance = differenceEuclidean("oklab");

/** Tokens that must match the reference closely. */
const WITHIN = 0.01;
/** Tokens the reference only aims at. */
const NEAR = 0.03;

const themes = {
  dark: generateTheme(themeInputs.dark),
  light: generateTheme(themeInputs.light),
};

const surfaces = ["canvas", "panel", "raised"] as const;

function expectNear(actual: Color, hex: string, tolerance: number) {
  expect(distance(actual, hex)).toBeLessThanOrEqual(tolerance);
}

/** The lowest WCAG contrast of a token against the theme's three surfaces. */
function lowestContrast(theme: Theme, token: keyof Theme["tokens"]) {
  return Math.min(
    ...surfaces.map((surface) =>
      wcagContrast(theme.tokens[token], theme.tokens[surface]),
    ),
  );
}

function expectFloors(theme: Theme) {
  for (const token of ["heading", "body", "muted"] as const) {
    expect(lowestContrast(theme, token), token).toBeGreaterThanOrEqual(4.5);
  }
  // Faint draws the marks and edges that carry meaning (nodes, rules, an
  // option's ring); the accent, focus and state colours mark state.
  for (const token of [
    "faint",
    "accent",
    "focus",
    "green",
    "amber",
    "red",
  ] as const) {
    expect(lowestContrast(theme, token), token).toBeGreaterThanOrEqual(3);
  }
  expect(
    wcagContrast(theme.tokens.primaryText, theme.tokens.primaryFill),
    "primaryText on primaryFill",
  ).toBeGreaterThanOrEqual(4.5);
}

describe("generateTheme", () => {
  it("tunes the dark theme to the reference's neutral ramp", () => {
    const { tokens } = themes.dark;
    expectNear(tokens.canvas, "#08090a", WITHIN);
    expectNear(tokens.panel, "#0f1011", WITHIN);
    expectNear(tokens.heading, "#f7f8f8", WITHIN);
    expectNear(tokens.muted, "#8a8f98", WITHIN);
    expectNear(tokens.body, "#d0d6e0", NEAR);
  });

  it("lands the light theme near the direction's table", () => {
    const { tokens } = themes.light;
    expectNear(tokens.canvas, "#fdfdfd", NEAR);
    expectNear(tokens.panel, "#f5f6f7", NEAR);
    expectNear(tokens.heading, "#0f1011", NEAR);
    expectNear(tokens.body, "#26282c", NEAR);
    expectNear(tokens.muted, "#5d6169", NEAR);
  });

  it("emits every colour token in OKLCH", () => {
    for (const theme of Object.values(themes)) {
      for (const token of Object.values(theme.tokens)) {
        expect(token.mode).toBe("oklch");
      }
    }
  });

  it("knows which theme is dark", () => {
    expect(themes.dark.scheme).toBe("dark");
    expect(themes.light.scheme).toBe("light");
  });

  it.each(["dark", "light"] as const)(
    "holds the contrast floors in the %s theme on canvas, panel and raised",
    (name) => expectFloors(themes[name]),
  );

  it("separates text further from the ground as contrast rises", () => {
    const standard = generateTheme(themeInputs.dark);
    const higher = generateTheme({ ...themeInputs.dark, contrast: 1.3 });
    for (const token of ["muted", "faint"] as const) {
      expect(lowestContrast(higher, token)).toBeGreaterThan(
        lowestContrast(standard, token),
      );
    }
  });

  it.each(["dark", "light"] as const)(
    "keeps the floors in the %s theme at the lowest contrast",
    (name) =>
      expectFloors(generateTheme({ ...themeInputs[name], contrast: 0 })),
  );
});

describe("deriveSurface", () => {
  it.each(["dark", "light"] as const)(
    "re-derives a %s raised surface from its own background",
    (name) => {
      const theme = themes[name];
      const raised = deriveSurface(theme, theme.tokens.raised);
      expect(
        distance(raised.tokens.canvas, theme.tokens.raised),
      ).toBeLessThanOrEqual(0.001);
      expect(raised.scheme).toBe(theme.scheme);
      expectFloors(raised);
    },
  );

  it("keeps muted text's standing against the new ground", () => {
    const theme = themes.dark;
    const raised = deriveSurface(theme, theme.tokens.raised);
    const own = wcagContrast(raised.tokens.muted, raised.tokens.canvas);
    const reference = wcagContrast(theme.tokens.muted, theme.tokens.canvas);
    expect(Math.abs(own / reference - 1)).toBeLessThanOrEqual(0.05);
    expect(distance(raised.tokens.muted, theme.tokens.muted)).toBeGreaterThan(
      0.005,
    );
  });
});
