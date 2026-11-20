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
  union,
} from "../drawing.testing.tsx";

afterEach(clearStyles);

const FIXTURE = JSON.parse(
  readFileSync(
    resolve(
      import.meta.dirname,
      "../../../../../crates/atlas-core/tests/fixtures/catalog/valid/layered-figure.json",
    ),
    "utf8",
  ),
) as unknown;

const LAYERS = [
  ["Crust", "Thin, cold and brittle"],
  ["Lithospheric mantle", "Rigid, moves with the crust"],
  ["Asthenosphere", "Hot enough to flow slowly"],
];

function layersOf(layers: { label: string; note: string }[]) {
  return specOf("layered-figure", { caption: "Layers", layers });
}

const LONG_NOTE =
  "Below the rigid lid the rock is hot enough, and under enough pressure, that over millions of years it flows like a very stiff fluid, carrying the plates above it along with the convection of the deeper mantle.";
const LONG_LABEL =
  "The partially molten, mechanically weak upper mantle beneath the lithosphere";

const MANY = layersOf(
  Array.from({ length: 10 }, (_, index) => ({
    label: `Layer ${index + 1}`,
    note: index % 2 === 0 ? LONG_NOTE : "Short.",
  })),
);

describe("the layered figure", () => {
  it("draws every layer's label and note, first layer on top", () => {
    const { svg } = drawVisual(FIXTURE);
    const texts = drawnTexts(svg);
    for (const [label, note] of LAYERS) {
      expect(texts, label).toContain(label);
      expect(texts, note).toContain(note);
    }
    const tops = LAYERS.map(([label]) =>
      union(textBoxes(groupTitled(svg, label))),
    );
    tops.reduce((above, below) => {
      expect(below.top).toBeGreaterThan(above.bottom);
      return below;
    });
  });

  it("draws one stacked face per layer, top to bottom, none overlapping", async () => {
    const { svg } = drawVisual(FIXTURE);
    await applyStyles("light");
    const faces = LAYERS.map(([label]) => {
      const group = groupTitled(svg, label);
      const shapes = [...group.querySelectorAll("path")].filter(
        (path) => getComputedStyle(path).fill !== "none",
      );
      expect(shapes.length, label).toBeGreaterThan(0);
      return shapes;
    });
    const spans = faces.map((shapes) => {
      const ys = shapes.flatMap((path) =>
        (path.getAttribute("d") ?? "")
          .split(/[^\d.-]+/)
          .filter(Boolean)
          .map(Number)
          .filter((_, index) => index % 2 === 1),
      );
      return { top: Math.min(...ys), bottom: Math.max(...ys) };
    });
    spans.reduce((above, below) => {
      expect(below.top).toBeGreaterThan(above.bottom);
      return below;
    });
  });

  it("draws the same spec the same way every time", () => {
    expect(geometryOf(drawVisual(structuredClone(FIXTURE)).svg)).toBe(
      geometryOf(drawVisual(FIXTURE).svg),
    );
  });

  it.each([
    [
      "two layers",
      layersOf([
        { label: "A", note: "B" },
        { label: "C", note: "D" },
      ]),
    ],
    ["ten layers with long notes", MANY],
    [
      "a long label",
      layersOf([
        { label: LONG_LABEL, note: LONG_NOTE },
        { label: "Supercalifragilisticexpialidociouslithosphere", note: "x" },
      ]),
    ],
  ])("keeps %s inside the frame with no text overlapping", (_, spec) => {
    const { svg } = drawVisual(spec);
    expectAllInside(svg);
    expect(overlapping(textBoxes(svg))).toEqual([]);
  });

  it("wraps or shortens a long label and keeps its full text", () => {
    const { svg } = drawVisual(
      layersOf([
        { label: LONG_LABEL, note: LONG_NOTE },
        { label: "Crust", note: "Thin" },
      ]),
    );
    const group = groupTitled(svg, LONG_LABEL);
    const lines = [...group.querySelectorAll("text")].flatMap(linesOf);
    expect(lines.length).toBeGreaterThan(2);
    const list = screen.getByRole("list", { name: "Layers, top first" });
    expect(inside(list).getAllByRole("listitem")[0]).toHaveTextContent(
      `${LONG_LABEL}: ${LONG_NOTE}`,
    );
  });

  it("lists its layers in order as its text alternative", () => {
    const { svg } = drawVisual(FIXTURE);
    expect(svg).toHaveAttribute("aria-hidden", "true");
    const list = screen.getByRole("list", { name: "Layers, top first" });
    expect(list.tagName).toBe("OL");
    expect(
      inside(list)
        .getAllByRole("listitem")
        .map((item) => item.textContent),
    ).toEqual(LAYERS.map(([label, note]) => `${label}: ${note}`));
  });

  it("shows its caption as its heading when shown alone", () => {
    drawVisual(FIXTURE);
    expect(
      screen.getByRole("heading", { name: "The layers under a continent" }),
    ).toBeVisible();
  });

  it("leaves the caption to the frame around it", () => {
    const { container } = drawVisual(FIXTURE, { framed: true });
    expect(screen.queryByRole("heading")).toBeNull();
    expect(container).not.toHaveTextContent("The layers under a continent");
  });

  describe.each(schemes)("in the %s theme", (scheme) => {
    it("draws in the text colour on panel and canvas faces", async () => {
      const { svg } = drawVisual(MANY);
      await applyStyles(scheme);
      expectFigurePaints(svg);
    });
  });
});
