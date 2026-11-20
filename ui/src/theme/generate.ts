import {
  clampChroma,
  converter,
  wcagContrast,
  type Color,
  type Oklch,
} from "culori";

/**
 * The theme generator of ADR-0011: a base colour, an accent and a contrast
 * value in, every colour token in OKLCH out. Lightness is solved against WCAG
 * contrast ratios, so the floors hold by construction for any input and on
 * any background a surface re-derives from.
 */

export type ThemeInput = {
  /** The ground the theme is built on; it becomes the canvas. */
  base: Color | string;
  /** The one saturated colour: position, marks and focus. */
  accent: Color | string;
  /**
   * How far text and marks stand from the ground. 1 reproduces the reference;
   * higher separates them further, lower brings them closer, never under the
   * contrast floors.
   */
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

export type Scheme = "light" | "dark";

export type Theme = {
  input: ThemeInput;
  scheme: Scheme;
  tokens: Record<TokenName, Oklch>;
};

/** The two input sets; the dark one is tuned to the reference's ramp. */
export const themeInputs = {
  dark: { base: "#08090a", accent: "#7a83e8", contrast: 1 },
  light: { base: "#fdfdfd", accent: "#5e6ad2", contrast: 1 },
} satisfies Record<Scheme, ThemeInput>;

/** Contrast of each surface against the canvas. */
const PANEL_STEP = 1.055;
const RAISED_STEP = 1.12;

/** Contrast against the canvas that each text token aims for at contrast 1. */
const TEXT_TARGETS = { heading: 18.7, body: 14, muted: 6.1, faint: 3.45 };
const PRIMARY_FILL_TARGET = 16;
const STATE_TARGET = 5;

/** WCAG floors on every surface: text, and marks that carry meaning. */
const TEXT_FLOOR = 4.5;
const MARK_FLOOR = 3;
/** Headroom over a floor, so rounding in CSS cannot dip under it. */
const MARGIN = 0.05;

/** Peak chroma of the neutral ramp, tinted toward the base or the accent. */
const NEUTRAL_TINT = 0.014;
/** A base with less chroma than this is treated as neutral. */
const TINTED_BASE = 0.02;

const STATE_HUES = { green: 150, amber: 75, red: 25 };
const STATE_CHROMA = 0.16;

/**
 * Translucent ink: dark grounds need a little more of it to read. Each fill
 * stays at least 0.02 from the hairline, so a fill and an edge never share a
 * value and a colour names one token.
 */
const ALPHAS = {
  dark: {
    wash: 0.05,
    washStrong: 0.1,
    edge: 0.08,
    edgeStrong: 0.14,
    accentWash: 0.14,
  },
  light: {
    wash: 0.04,
    washStrong: 0.06,
    edge: 0.08,
    edgeStrong: 0.16,
    accentWash: 0.1,
  },
} satisfies Record<Scheme, Record<string, number>>;

const toOklch = converter("oklch");

function parse(color: Color | string): Oklch {
  const parsed = toOklch(color);
  if (!parsed) throw new Error(`Not a colour: ${String(color)}`);
  return parsed;
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

/** A colour at lightness `l`, its chroma reduced until it fits sRGB. */
function atLightness(l: number, c: number, h: number | undefined): Oklch {
  return clampChroma({ mode: "oklch", l, c, h }, "oklch");
}

/** Scales a target ratio by the contrast input, keeping 1 as "no contrast". */
const scaled = (ratio: number, contrast: number) => 1 + (ratio - 1) * contrast;

const lowestContrast = (color: Oklch, grounds: Oklch[]) =>
  Math.min(...grounds.map((ground) => wcagContrast(color, ground)));

/**
 * The lightness closest to `from`, moving toward `toward`, at which the
 * colour reaches `ratio` against every ground; `toward` if none does.
 */
function solveLightness(
  make: (l: number) => Oklch,
  grounds: Oklch[],
  ratio: number,
  from: number,
  toward: number,
): number {
  if (lowestContrast(make(toward), grounds) < ratio) return toward;
  let near = from;
  let far = toward;
  for (let step = 0; step < 40; step += 1) {
    const middle = (near + far) / 2;
    if (lowestContrast(make(middle), grounds) >= ratio) far = middle;
    else near = middle;
  }
  return far;
}

export function generateTheme(input: ThemeInput): Theme {
  const canvas = parse(input.base);
  const accentInput = parse(input.accent);
  const scheme: Scheme =
    wcagContrast(canvas, "#ffffff") >= wcagContrast(canvas, "#000000")
      ? "dark"
      : "light";
  const ink = scheme === "dark" ? 1 : 0;
  const further = (a: number, b: number) =>
    ink === 1 ? Math.max(a, b) : Math.min(a, b);

  const surface = (l: number) => atLightness(l, canvas.c, canvas.h);
  const panel = surface(
    solveLightness(surface, [canvas], PANEL_STEP, canvas.l, ink),
  );
  const raised = surface(
    solveLightness(surface, [canvas], RAISED_STEP, canvas.l, 1),
  );
  const surfaces = [canvas, panel, raised];

  const hue = canvas.c >= TINTED_BASE ? canvas.h : accentInput.h;
  const neutral = (l: number) =>
    atLightness(
      l,
      NEUTRAL_TINT * clamp01((1 - l) / 0.15) * clamp01(l / 0.4),
      hue,
    );

  /**
   * Solves a colour family to its target against the canvas, then pushes it
   * further if any surface would leave it under its floor.
   */
  const solve = (make: (l: number) => Oklch, target: number, floor: number) =>
    make(
      further(
        solveLightness(make, [canvas], target, canvas.l, ink),
        solveLightness(make, surfaces, floor + MARGIN, canvas.l, ink),
      ),
    );

  const text = (name: keyof typeof TEXT_TARGETS) =>
    solve(
      neutral,
      scaled(TEXT_TARGETS[name], input.contrast),
      name === "faint" ? MARK_FLOOR : TEXT_FLOOR,
    );

  const heading = text("heading");

  const accent =
    lowestContrast(accentInput, surfaces) >= MARK_FLOOR + MARGIN
      ? accentInput
      : solve(
          (l) => atLightness(l, accentInput.c, accentInput.h),
          MARK_FLOOR,
          MARK_FLOOR,
        );

  const state = (hueAngle: number) =>
    solve(
      (l) => atLightness(l, STATE_CHROMA, hueAngle),
      scaled(STATE_TARGET, input.contrast),
      MARK_FLOOR,
    );

  const primaryFill = solve(
    neutral,
    Math.max(PRIMARY_FILL_TARGET, scaled(PRIMARY_FILL_TARGET, input.contrast)),
    TEXT_FLOOR,
  );
  // The surface that stands furthest from the fill carries its label.
  const primaryText =
    wcagContrast(canvas, primaryFill) >= wcagContrast(raised, primaryFill)
      ? canvas
      : raised;

  const alphas = ALPHAS[scheme];
  const translucent = (color: Oklch, alpha: number): Oklch => ({
    ...color,
    alpha,
  });

  return {
    input,
    scheme,
    tokens: {
      canvas,
      panel,
      raised,
      heading,
      body: text("body"),
      muted: text("muted"),
      faint: text("faint"),
      edge: translucent(heading, alphas.edge),
      edgeStrong: translucent(heading, alphas.edgeStrong),
      wash: translucent(heading, alphas.wash),
      washStrong: translucent(heading, alphas.washStrong),
      accent,
      accentWash: translucent(accent, alphas.accentWash),
      primaryFill,
      primaryText,
      focus: accent,
      green: state(STATE_HUES.green),
      amber: state(STATE_HUES.amber),
      red: state(STATE_HUES.red),
    },
  };
}

/**
 * Regenerates a theme with a surface's own background as its base, the way a
 * menu, popover or raised panel re-derives its labels, edges and controls.
 */
export function deriveSurface(theme: Theme, background: Color | string): Theme {
  return generateTheme({ ...theme.input, base: background });
}
