import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { screen, within as inside } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { applyStyles, clearStyles, schemes } from "@/test/styles";

import {
  drawnTexts,
  drawVisual,
  expectAllInside,
  expectFigurePaints,
  geometryOf,
  groupTitled,
  linesOf,
  overlapping,
  specOf,
  textBoxes,
} from "../drawing.testing.tsx";

afterEach(clearStyles);

const FIXTURE = JSON.parse(
  readFileSync(
    resolve(
      import.meta.dirname,
      "../../../../../crates/atlas-core/tests/fixtures/catalog/valid/step-through.json",
    ),
    "utf8",
  ),
) as unknown;

const STEPS = [
  ["The slab sinks", "The denser plate descends into the mantle"],
  ["Water is released", "Heat drives water out of the slab"],
  ["Rock melts", "Water lowers the melting point of the mantle above"],
];

function stepsOf(steps: { label: string; note: string }[]) {
  return specOf("step-through", { caption: "Steps", steps });
}

const LONG_NOTE =
  "Water carried down in the minerals of the slab is driven out as it heats, rising into the wedge of mantle above, where it lowers the melting point of the rock until part of it melts and rises.";
const LONG_LABEL =
  "Dehydration of hydrous minerals in the descending oceanic lithosphere";

const MANY = stepsOf(
  Array.from({ length: 9 }, (_, index) => ({
    label: `Step label ${index + 1}`,
    note: index % 2 === 0 ? LONG_NOTE : "Short.",
  })),
);

/** The centre of each numbered marker, in the order of the steps. */
function markers(svg: SVGSVGElement, count: number) {
  return Array.from({ length: count }, (_, index) => {
    const circle = groupTitled(svg, `Step ${index + 1}`).querySelector(
      "circle",
    );
    expect(circle, `the marker of step ${index + 1}`).not.toBeNull();
    return {
      x: Number(circle!.getAttribute("cx")),
      y: Number(circle!.getAttribute("cy")),
    };
  });
}

/** The straight line segments of every path in the drawing. */
function segments(svg: SVGSVGElement) {
  return [...svg.querySelectorAll("path")].flatMap((path) => {
    const commands = (path.getAttribute("d") ?? "").match(
      /[ML]\s*-?[\d.]+[\s,]+-?[\d.]+/g,
    );
    const points = (commands ?? []).map((command) =>
      command
        .slice(1)
        .trim()
        .split(/[\s,]+/)
        .map(Number),
    );
    return points
      .slice(1)
      .map(([x, y], index) => ({ from: points[index], to: [x, y] }));
  });
}

function onSegment(
  point: { x: number; y: number },
  { from, to }: { from: number[]; to: number[] },
) {
  const [x1, y1] = from;
  const [x2, y2] = to;
  const cross = (x2 - x1) * (point.y - y1) - (y2 - y1) * (point.x - x1);
  const within =
    point.x >= Math.min(x1, x2) - 0.01 &&
    point.x <= Math.max(x1, x2) + 0.01 &&
    point.y >= Math.min(y1, y2) - 0.01 &&
    point.y <= Math.max(y1, y2) + 0.01;
  return Math.abs(cross) < 0.01 && within;
}

describe("the step-through", () => {
  it.each([
    ["the fixture", FIXTURE, STEPS],
    [
      "nine steps",
      MANY,
      Array.from({ length: 9 }, (_, index) => [
        `Step label ${index + 1}`,
        index % 2 === 0 ? LONG_NOTE : "Short.",
      ]),
    ],
  ])(
    "draws every step of %s numbered in order, labels and notes in reading order",
    (_, spec, steps) => {
      const { svg } = drawVisual(spec);
      const texts = drawnTexts(svg);
      steps.forEach(([label, note], index) => {
        const group = groupTitled(svg, `Step ${index + 1}`);
        expect(drawnTexts(group)).toContain(String(index + 1));
        expect(texts).toContain(label);
        if (note !== LONG_NOTE) expect(texts).toContain(note);
      });
      expect(overlapping(textBoxes(svg))).toEqual([]);
    },
  );

  it.each([
    ["three steps", FIXTURE, 3],
    ["nine steps", MANY, 9],
  ])("joins the %s by one line through every marker", (_, spec, count) => {
    const { svg } = drawVisual(spec);
    const centres = markers(svg, count);
    const joining = segments(svg).filter((segment) =>
      centres.every((centre) => onSegment(centre, segment)),
    );
    expect(joining.length).toBeGreaterThan(0);
  });

  it("shows every step at once, with no control to step through", () => {
    const { container, svg } = drawVisual(FIXTURE);
    markers(svg, 3);
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(container.querySelectorAll("button, a, [tabindex]")).toHaveLength(0);
  });

  it("draws the same spec the same way every time", () => {
    expect(geometryOf(drawVisual(structuredClone(FIXTURE)).svg)).toBe(
      geometryOf(drawVisual(FIXTURE).svg),
    );
    expect(geometryOf(drawVisual(MANY).svg)).toBe(
      geometryOf(drawVisual(MANY).svg),
    );
  });

  it.each([
    [
      "two steps",
      stepsOf([
        { label: "A", note: "B" },
        { label: "C", note: "D" },
      ]),
    ],
    ["nine steps with long notes", MANY],
    [
      "long labels",
      stepsOf([
        { label: LONG_LABEL, note: LONG_NOTE },
        { label: "Supercalifragilisticexpialidociousmagma", note: "x" },
        { label: "Rock melts", note: LONG_NOTE },
      ]),
    ],
  ])("keeps %s inside the frame with no text overlapping", (_, spec) => {
    const { svg } = drawVisual(spec);
    expectAllInside(svg);
    expect(overlapping(textBoxes(svg))).toEqual([]);
  });

  it("wraps or shortens a long label and keeps its full text", () => {
    const { svg } = drawVisual(
      stepsOf([
        { label: LONG_LABEL, note: "Short." },
        { label: "Rock melts", note: "Short." },
      ]),
    );
    const label = [...groupTitled(svg, "Step 1").querySelectorAll("text")]
      .flatMap(linesOf)
      .filter((line) => line.text !== "1" && line.text !== "Short.");
    expect(label.length).toBeGreaterThan(1);
    const list = screen.getByRole("list", { name: "Steps" });
    expect(inside(list).getAllByRole("listitem")[0]).toHaveTextContent(
      `${LONG_LABEL}: Short.`,
    );
  });

  it("lists its steps in order as its text alternative", () => {
    const { svg } = drawVisual(FIXTURE);
    expect(svg).toHaveAttribute("aria-hidden", "true");
    const list = screen.getByRole("list", { name: "Steps" });
    expect(list.tagName).toBe("OL");
    expect(
      inside(list)
        .getAllByRole("listitem")
        .map((item) => item.textContent),
    ).toEqual(STEPS.map(([label, note]) => `${label}: ${note}`));
  });

  it("shows its caption as its heading when shown alone", () => {
    drawVisual(FIXTURE);
    expect(
      screen.getByRole("heading", { name: "How a volcanic arc forms" }),
    ).toBeVisible();
  });

  it("leaves the caption to the frame around it", () => {
    const { container } = drawVisual(FIXTURE, { framed: true });
    expect(screen.queryByRole("heading")).toBeNull();
    expect(container).not.toHaveTextContent("How a volcanic arc forms");
  });

  describe.each(schemes)("in the %s theme", (scheme) => {
    it("draws in the text colour on panel and canvas faces", async () => {
      const { svg } = drawVisual(MANY);
      await applyStyles(scheme);
      expectFigurePaints(svg);
    });
  });
});
