/**
 * Reading a drawn catalog visual in jsdom, which lays nothing out: boxes come
 * from the SVG's own geometry, and the extent of a line of text from the
 * figure type's metrics.
 */
import { render } from "@testing-library/react";
import { expect } from "vitest";

import { tokenOf } from "@/test/styles";

import { CatalogVisual } from "../catalog-visual.tsx";
import { CaptionInFrame } from "../frame-context.ts";

/** Figure labels: 12 px type. */
export const FONT_SIZE = 12;

/**
 * The advance of one character of a figure label: IBM Plex Mono's glyphs are
 * 600 units of a 1000-unit em wide, and the labels are tracked 0.04 em.
 */
export const ADVANCE = FONT_SIZE * 0.6 + FONT_SIZE * 0.04;

/**
 * How far a line of text reaches above and below its baseline: Plex Mono's
 * ascender is 1025 units and its descender 275.
 */
const ASCENT = FONT_SIZE * 1.025;
const DESCENT = FONT_SIZE * 0.275;

export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** Renders a catalog spec through `CatalogVisual`, in a frame or alone. */
export function drawVisual(spec: unknown, { framed = false } = {}) {
  const onInvalid = (reasons: string[]) => {
    throw new Error(`invalid spec: ${reasons.join("; ")}`);
  };
  const visual = <CatalogVisual spec={spec} onInvalid={onInvalid} />;
  const { container } = render(
    <CaptionInFrame value={framed}>{visual}</CaptionInFrame>,
  );
  const svg = container.querySelector("svg");
  if (!svg) throw new Error("the visual draws no SVG");
  return { container, svg };
}

/** A catalog spec holding one element. */
export function specOf(type: string, props: Record<string, unknown>) {
  return { root: "visual", elements: { visual: { type, props } } };
}

function number(element: Element, name: string): number {
  const value = element.getAttribute(name);
  if (value === null) throw new Error(`<${element.tagName}> has no ${name}`);
  return Number(value);
}

/** The drawing's coordinate box. */
export function viewBoxOf(svg: SVGSVGElement): Box {
  const [x, y, width, height] = (svg.getAttribute("viewBox") ?? "")
    .trim()
    .split(/[\s,]+/)
    .map(Number);
  return { left: x, top: y, right: x + width, bottom: y + height };
}

/** The box of a `<rect>`. */
export function rectBox(rect: Element): Box {
  const x = number(rect, "x");
  const y = number(rect, "y");
  return {
    left: x,
    top: y,
    right: x + number(rect, "width"),
    bottom: y + number(rect, "height"),
  };
}

/** The box of a `<circle>`. */
export function circleBox(circle: Element): Box {
  const cx = number(circle, "cx");
  const cy = number(circle, "cy");
  const r = number(circle, "r");
  return { left: cx - r, top: cy - r, right: cx + r, bottom: cy + r };
}

export interface Line {
  text: string;
  box: Box;
}

/**
 * The lines of a `<text>`: each `<tspan>` with its own position, or the text
 * itself, with the box each covers as drawn.
 */
export function linesOf(text: Element): Line[] {
  const anchor = text.getAttribute("text-anchor") ?? "start";
  const parts = text.querySelectorAll("tspan");
  const lines = parts.length > 0 ? [...parts] : [text];
  return lines.map((line) => {
    const content = line.textContent ?? "";
    const width = [...content].length * ADVANCE;
    const x = number(line, "x");
    const y = number(line, "y");
    const left =
      anchor === "middle" ? x - width / 2 : anchor === "end" ? x - width : x;
    return {
      text: content,
      box: { left, top: y - ASCENT, right: left + width, bottom: y + DESCENT },
    };
  });
}

/** Every text element's lines joined as it reads. */
export function drawnTexts(svg: Element): string[] {
  return [...svg.querySelectorAll("text")].map((text) =>
    linesOf(text)
      .map((line) => line.text)
      .join(" "),
  );
}

/** The box around every line of every text in `root`. */
export function textBoxes(root: Element): Box[] {
  return [...root.querySelectorAll("text")].flatMap((text) =>
    linesOf(text).map((line) => line.box),
  );
}

/** The box around a set of boxes. */
export function union(boxes: Box[]): Box {
  return {
    left: Math.min(...boxes.map((box) => box.left)),
    top: Math.min(...boxes.map((box) => box.top)),
    right: Math.max(...boxes.map((box) => box.right)),
    bottom: Math.max(...boxes.map((box) => box.bottom)),
  };
}

const EPSILON = 0.01;

export function within(inner: Box, outer: Box): boolean {
  return (
    inner.left >= outer.left - EPSILON &&
    inner.top >= outer.top - EPSILON &&
    inner.right <= outer.right + EPSILON &&
    inner.bottom <= outer.bottom + EPSILON
  );
}

export function overlap(a: Box, b: Box): boolean {
  return (
    a.left < b.right - EPSILON &&
    b.left < a.right - EPSILON &&
    a.top < b.bottom - EPSILON &&
    b.top < a.bottom - EPSILON
  );
}

/** Pairs of boxes that overlap, by index. */
export function overlapping(boxes: Box[]): [number, number][] {
  const pairs: [number, number][] = [];
  boxes.forEach((a, i) =>
    boxes.slice(i + 1).forEach((b, offset) => {
      if (overlap(a, b)) pairs.push([i, i + 1 + offset]);
    }),
  );
  return pairs;
}

/** Expects every drawn shape and line of text inside the drawing's box. */
export function expectAllInside(svg: SVGSVGElement) {
  const frame = viewBoxOf(svg);
  expect(frame.right - frame.left).toBeGreaterThan(0);
  expect(frame.bottom - frame.top).toBeGreaterThan(0);
  const shapes = [
    ...[...svg.querySelectorAll("rect")].map(rectBox),
    ...[...svg.querySelectorAll("circle")].map(circleBox),
    ...textBoxes(svg),
  ];
  expect(shapes.length).toBeGreaterThan(0);
  const outside = shapes.filter((box) => !within(box, frame));
  expect(outside, JSON.stringify(frame)).toEqual([]);
}

/** The `<g>` whose own `<title>` reads `title`. */
export function groupTitled(svg: Element, title: string): Element {
  const found = [...svg.querySelectorAll("g")].filter(
    (group) => group.querySelector(":scope > title")?.textContent === title,
  );
  expect(found, `a group titled "${title}"`).toHaveLength(1);
  return found[0];
}

/**
 * The drawing as geometry and text, with generated ids left out: equal for
 * two renderings that draw the same thing in the same place.
 */
export function geometryOf(svg: Element): string {
  return [svg, ...svg.querySelectorAll("*")]
    .map((element) => {
      const attributes = [...element.attributes]
        .filter(({ name }) => name !== "id" && name !== "class")
        .map(
          ({ name, value }) =>
            `${name}=${value.replaceAll(/url\(#[^)]*\)/g, "url(#)")}`,
        );
      const text = [...element.childNodes]
        .filter((node) => node.nodeType === Node.TEXT_NODE)
        .map((node) => node.textContent)
        .join("");
      return `${element.tagName} ${attributes.join(" ")} ${text}`;
    })
    .join("\n");
}

const PAINTED =
  "rect, path, circle, line, polyline, polygon, ellipse, text, tspan";

/** How each painted element fills and strokes, as theme tokens. */
export function paintTokens(svg: Element) {
  return [...svg.querySelectorAll(PAINTED)].map((element) => {
    const style = getComputedStyle(element);
    const token = (paint: string) => {
      if (paint === "none" || paint === "") return "none";
      const color = paint === "currentcolor" ? style.color : paint;
      return tokenOf(color, element) ?? `literal ${color}`;
    };
    return {
      element: element.tagName.toLowerCase(),
      fill: token(style.fill),
      stroke: token(style.stroke),
    };
  });
}

/**
 * Expects a drawing painted only with the figure's tokens: strokes in the
 * text colour, faces in the panel, canvas or raised tokens, text in the body
 * or muted colour, and nothing else.
 */
export function expectFigurePaints(svg: Element) {
  const paints = paintTokens(svg);
  expect(paints.length).toBeGreaterThan(0);
  for (const paint of paints) {
    const where = JSON.stringify(paint);
    if (paint.element === "text" || paint.element === "tspan") {
      expect(["body", "muted"], where).toContain(paint.fill);
      expect(paint.stroke, where).toBe("none");
      continue;
    }
    expect(["none", "panel", "canvas", "raised", "body"], where).toContain(
      paint.fill,
    );
    expect(["none", "body"], where).toContain(paint.stroke);
  }
}
