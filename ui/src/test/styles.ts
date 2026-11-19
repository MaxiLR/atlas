/**
 * The app's real styles in jsdom, so a test can read what a component paints.
 *
 * Tailwind compiles `src/index.css` for the classes rendered right now, the
 * theme generator supplies both themes' tokens as the Vite plugin would, and
 * jsdom resolves the cascade, `var()` and `color-mix()` in `getComputedStyle`.
 * jsdom cannot hover or press, so those states are forced with a class.
 */
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";

import { fireEvent } from "@testing-library/react";

import { converter, differenceEuclidean } from "culori";
import { compile } from "tailwindcss";

import { generateTheme, themeInputs, type Scheme } from "@/theme/generate";
import { themeStylesheet } from "@/theme/stylesheet";

export const schemes = ["light", "dark"] as const satisfies Scheme[];

/** States jsdom cannot enter, forced by adding the class to the element. */
export const forced = {
  hover: "force-hover",
  active: "force-active",
} as const;

const src = join(import.meta.dirname, "..");
const nodeModules = join(src, "../node_modules");
const requireFromSrc = createRequire(join(src, "index.css"));

async function locate(id: string, base: string): Promise<string> {
  if (id.startsWith(".")) return resolve(base, id);
  if (id === "tailwindcss")
    return requireFromSrc.resolve("tailwindcss/index.css");
  try {
    return requireFromSrc.resolve(id);
  } catch {
    // A package that only declares a stylesheet, under `style`.
    const folder = join(nodeModules, id);
    const manifest = JSON.parse(
      await readFile(join(folder, "package.json"), "utf8"),
    ) as { style?: string; main?: string };
    return join(folder, manifest.style ?? manifest.main ?? "index.css");
  }
}

/** The part of Tailwind's compiler the tests use. */
interface Compiler {
  build(candidates: string[]): string;
}
let compiler: Promise<Compiler> | undefined;

function tailwind(): Promise<Compiler> {
  compiler ??= readFile(join(src, "index.css"), "utf8").then((css) =>
    compile(css, {
      base: src,
      async loadStylesheet(id, base) {
        const path = await locate(id, base);
        return {
          path,
          base: dirname(path),
          content: await readFile(path, "utf8"),
        };
      },
    }),
  );
  return compiler;
}

/** Hover and press as classes; focus-visible as the focus a test gives. */
function forceable(css: string) {
  return css
    .replaceAll("@media (hover: hover)", "@media all")
    .replaceAll(":hover", `.${forced.hover}`)
    .replaceAll(":active", `.${forced.active}`)
    .replaceAll(":focus-visible", ":focus");
}

const STYLE_ID = "atlas-test-styles";

/**
 * Shows the document in `scheme` with the app's styles for every class it
 * holds now. Call it again after the document gains classes (a menu opens).
 */
export async function applyStyles(scheme: Scheme): Promise<void> {
  document.documentElement.dataset.theme = scheme;
  const classes = new Set<string>();
  for (const element of document.querySelectorAll("[class]")) {
    for (const name of element.classList) classes.add(name);
  }
  const css = (await tailwind()).build([...classes]);
  let style = document.getElementById(STYLE_ID);
  if (!style) {
    style = document.createElement("style");
    style.id = STYLE_ID;
    document.head.append(style);
  }
  style.textContent = `${themeStylesheet()}\n${forceable(css)}`;
}

const tokenNames = Object.keys(generateTheme(themeInputs.dark).tokens).map(
  (name) => name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`),
);

/** The theme's tokens as they apply at an element: `edge-strong` → value. */
export function tokensAt(element: Element): Map<string, string> {
  const style = getComputedStyle(element);
  return new Map(
    tokenNames.map((name) => [
      name,
      style.getPropertyValue(`--atlas-${name}`).trim(),
    ]),
  );
}

const toOklab = converter("oklab");
const distance = differenceEuclidean("oklab");

/** The element and every theme or surface scope above it. */
function scopesOf(element: Element): Element[] {
  const scopes = [element];
  for (
    let node = element.parentElement?.closest("[data-theme], [data-surface]");
    node;
    node = node.parentElement?.closest("[data-theme], [data-surface]")
  ) {
    scopes.push(node);
  }
  return scopes;
}

/**
 * The token a computed colour comes from, in the scope of `element` or of a
 * scope above it it may inherit from: `transparent`, a token at its own
 * opacity, or an opaque token at a lower opacity. Undefined for anything
 * else, which is a literal colour.
 */
export function tokenOf(color: string, element: Element): string | undefined {
  const parsed = toOklab(color);
  if (!parsed) return undefined;
  const alpha = parsed.alpha ?? 1;
  if (alpha === 0) return "transparent";
  const candidates = scopesOf(element).flatMap((scope) => [...tokensAt(scope)]);
  const matches = (exact: boolean) =>
    candidates.find(([, value]) => {
      const token = toOklab(value);
      if (!token) return false;
      const tokenAlpha = token.alpha ?? 1;
      const sameInk =
        distance({ ...parsed, alpha: 1 }, { ...token, alpha: 1 }) < 0.002;
      return (
        sameInk &&
        (exact ? Math.abs(tokenAlpha - alpha) < 0.01 : tokenAlpha === 1)
      );
    })?.[0];
  // A token at its own opacity names the colour before a faded opaque one.
  return matches(true) ?? matches(false);
}

/** The value of one token where the element is. */
export function token(element: Element, name: string): string {
  return getComputedStyle(element).getPropertyValue(`--atlas-${name}`).trim();
}

/** Whether two colours are the same, whatever notation they are in. */
export function sameColor(a: string, b: string): boolean {
  const left = toOklab(a);
  const right = toOklab(b);
  if (!left || !right) return false;
  return (
    distance(left, right) < 0.002 &&
    Math.abs((left.alpha ?? 1) - (right.alpha ?? 1)) < 0.01
  );
}

export type Paint = {
  color: string;
  background: string;
  border?: string;
  outline?: string;
};

/** The colours an element paints with: text, fill, edge and outline. */
export function paintOf(element: Element): Paint {
  const style = getComputedStyle(element);
  const paint: Paint = {
    color: style.color,
    background: style.backgroundColor,
  };
  if (style.borderTopStyle !== "none" && parseFloat(style.borderTopWidth) > 0)
    paint.border = style.borderTopColor;
  if (style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0)
    paint.outline = style.outlineColor;
  return paint;
}

/** Each colour the element paints with, named by its token. */
export function tokenPaintOf(element: Element): Record<string, string> {
  return Object.fromEntries(
    Object.entries(paintOf(element)).map(([part, color]) => [
      part,
      tokenOf(color, element) ?? `literal ${color}`,
    ]),
  );
}

/** The literal colours an element paints with; empty when all are tokens. */
export function literalColors(element: Element): string[] {
  return Object.entries(paintOf(element))
    .filter(([, color]) => tokenOf(color, element) === undefined)
    .map(([part, color]) => `${part}: ${color}`);
}

/** Literal colours anywhere in a subtree, with the element they are on. */
export function literalColorsIn(root: Element): string[] {
  return [root, ...root.querySelectorAll("*")].flatMap((element) =>
    literalColors(element).map(
      (literal) =>
        `${element.tagName.toLowerCase()}${element.getAttribute("data-slot") ? `[${element.getAttribute("data-slot")}]` : ""} ${literal}`,
    ),
  );
}

/** Resets what `applyStyles` changed. */
export function clearStyles(): void {
  document.getElementById(STYLE_ID)?.remove();
  delete document.documentElement.dataset.theme;
}

/**
 * Ends the element's running CSS animation as a browser would. jsdom runs no
 * animations and has no `AnimationEvent`, so a surface that animates out
 * would otherwise never unmount.
 */
export function endAnimation(element: Element): void {
  const event = new Event("animationend", { bubbles: true });
  Object.assign(event, {
    animationName: getComputedStyle(element).animationName,
  });
  fireEvent(element, event);
}
