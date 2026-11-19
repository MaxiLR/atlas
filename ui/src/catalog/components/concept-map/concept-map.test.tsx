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
  rectBox,
  specOf,
  within,
  type Box,
} from "../drawing.testing.tsx";

afterEach(clearStyles);

const FIXTURE = JSON.parse(
  readFileSync(
    resolve(
      import.meta.dirname,
      "../../../../../crates/atlas-core/tests/fixtures/catalog/valid/concept-map.json",
    ),
    "utf8",
  ),
) as unknown;

interface Node {
  id: string;
  label: string;
}
interface Link {
  from: string;
  to: string;
  label: string;
  directed?: boolean;
}

function mapOf(nodes: Node[], links: Link[]) {
  return specOf("concept-map", { caption: "A map", nodes, links });
}

const TWELVE = [
  "Mantle convection",
  "Ridge push",
  "Slab pull",
  "Oceanic plate",
  "Continental plate",
  "Subduction zone",
  "Volcanic arc",
  "Trench",
  "Earthquakes",
  "Mountain belt",
  "Sea-floor spreading",
  "Magnetic stripes",
].map((label, index) => ({ id: `n${index}`, label }));

/** Twelve nodes in three shapes: a hub, a chain and a dense web. */
const TWELVE_NODE_MAPS = {
  hub: mapOf(
    TWELVE,
    TWELVE.slice(1).map((node) => ({
      from: "n0",
      to: node.id,
      label: "drives",
      directed: true,
    })),
  ),
  chain: mapOf(
    TWELVE,
    TWELVE.slice(1).map((node, index) => ({
      from: TWELVE[index].id,
      to: node.id,
      label: "leads to",
    })),
  ),
  web: mapOf(
    TWELVE,
    TWELVE.flatMap((node, index) =>
      [1, 3, 5]
        .map((step) => TWELVE[(index + step) % TWELVE.length])
        .filter((_, at) => (index + at) % 2 === 0)
        .map((other) => ({
          from: node.id,
          to: other.id,
          label: "relates to",
          directed: index % 3 === 0,
        })),
    ),
  ),
};

/** The face drawn for each node, by its label. */
function nodeBoxes(svg: SVGSVGElement, labels: string[]): Box[] {
  return labels.map((label) => {
    const group = groupTitled(svg, label);
    const face = group.querySelector("rect");
    expect(face, `the face of "${label}"`).not.toBeNull();
    return rectBox(face!);
  });
}

const LONG_LABEL =
  "The asthenosphere's partially molten upper layer beneath the rigid lithosphere, which flows slowly over millions of years";

describe("the concept map", () => {
  it("draws every node and every link label of the fixture", () => {
    const { svg } = drawVisual(FIXTURE);
    const texts = drawnTexts(svg);
    for (const label of [
      "Oceanic plate",
      "Continental plate",
      "Trench",
      "sinks beneath",
      "bends into",
    ]) {
      expect(texts, label).toContain(label);
    }
    nodeBoxes(svg, ["Oceanic plate", "Continental plate", "Trench"]);
  });

  it("draws the same spec the same way every time", () => {
    const first = geometryOf(drawVisual(FIXTURE).svg);
    const again = geometryOf(drawVisual(structuredClone(FIXTURE)).svg);
    expect(again).toBe(first);
    const web = geometryOf(drawVisual(TWELVE_NODE_MAPS.web).svg);
    expect(geometryOf(drawVisual(TWELVE_NODE_MAPS.web).svg)).toBe(web);
  });

  it.each([
    ["the fixture", FIXTURE, ["Oceanic plate", "Continental plate", "Trench"]],
    ...Object.entries(TWELVE_NODE_MAPS).map(
      ([shape, spec]) =>
        [
          `twelve nodes as a ${shape}`,
          spec,
          TWELVE.map((node) => node.label),
        ] as const,
    ),
  ])(
    "keeps the node labels of %s apart, each inside its face",
    (_, spec, labels) => {
      const { svg } = drawVisual(spec);
      const faces = nodeBoxes(svg, [...labels]);
      expect(overlapping(faces)).toEqual([]);
      labels.forEach((label, index) => {
        const text = groupTitled(svg, label).querySelector("text")!;
        for (const line of linesOf(text)) {
          expect(within(line.box, faces[index]), line.text).toBe(true);
        }
      });
    },
  );

  it.each([
    [
      "a minimal map",
      mapOf(
        [
          { id: "a", label: "A" },
          { id: "b", label: "B" },
        ],
        [{ from: "a", to: "b", label: "c" }],
      ),
    ],
    ...Object.entries(TWELVE_NODE_MAPS).map(
      ([shape, spec]) => [`twelve nodes as a ${shape}`, spec] as const,
    ),
    [
      "long labels",
      mapOf(
        [
          { id: "a", label: LONG_LABEL },
          { id: "b", label: "Supercalifragilisticexpialidocious-lithosphere" },
          { id: "c", label: "Crust" },
        ],
        [
          { from: "a", to: "b", label: LONG_LABEL, directed: true },
          { from: "b", to: "c", label: "x" },
        ],
      ),
    ],
  ])("keeps %s inside the frame", (_, spec) => {
    expectAllInside(drawVisual(spec).svg);
  });

  it("wraps or shortens a long label and keeps its full text", () => {
    const { svg } = drawVisual(
      mapOf(
        [
          { id: "a", label: LONG_LABEL },
          { id: "b", label: "Crust" },
        ],
        [{ from: "a", to: "b", label: LONG_LABEL }],
      ),
    );
    const node = groupTitled(svg, LONG_LABEL);
    const lines = linesOf(node.querySelector("text")!);
    expect(lines.map((line) => line.text).join(" ")).not.toBe(LONG_LABEL);
    expect(lines.length).toBeGreaterThan(1);

    const link = groupTitled(svg, `${LONG_LABEL} ${LONG_LABEL} Crust`);
    expect(link.querySelector("text")).not.toBeNull();

    const concepts = screen.getByRole("list", { name: "Concepts" });
    expect(inside(concepts).getByText(LONG_LABEL)).toBeInTheDocument();
  });

  it("puts an arrowhead on directed links only, at the node they point to", () => {
    const { svg } = drawVisual(FIXTURE);

    const directed = groupTitled(
      svg,
      "Oceanic plate sinks beneath Continental plate",
    ).querySelector("path")!;
    const marker = directed.getAttribute("marker-end") ?? "";
    expect(marker).toMatch(/^url\(#.+\)$/);
    const id = marker.slice(5, -1);
    expect(svg.querySelector(`marker[id="${CSS.escape(id)}"] path`)).not.toBe(
      null,
    );
    const end = (directed.getAttribute("d") ?? "")
      .trim()
      .split(/[\s,A-Za-z]+/)
      .filter(Boolean)
      .map(Number)
      .slice(-2);
    const [target] = nodeBoxes(svg, ["Continental plate"]);
    const [x, y] = end;
    expect(x).toBeGreaterThanOrEqual(target.left);
    expect(x).toBeLessThanOrEqual(target.right);
    expect([target.top, target.bottom]).toContainEqual(y);

    const undirected = groupTitled(
      svg,
      "Oceanic plate bends into Trench",
    ).querySelector("path")!;
    expect(undirected.hasAttribute("marker-end")).toBe(false);
    expect(undirected.hasAttribute("marker-start")).toBe(false);
  });

  it("lists its concepts and links in order as its text alternative", () => {
    const { svg } = drawVisual(FIXTURE);
    expect(svg).toHaveAttribute("aria-hidden", "true");

    const concepts = screen.getByRole("list", { name: "Concepts" });
    expect(
      inside(concepts)
        .getAllByRole("listitem")
        .map((item) => item.textContent),
    ).toEqual(["Oceanic plate", "Continental plate", "Trench"]);

    const links = screen.getByRole("list", { name: "Links" });
    expect(
      inside(links)
        .getAllByRole("listitem")
        .map((item) => item.textContent),
    ).toEqual([
      "Oceanic plate sinks beneath Continental plate",
      "Oceanic plate bends into Trench",
    ]);
  });

  it("is interactive in no way", () => {
    const { container } = drawVisual(TWELVE_NODE_MAPS.web);
    expect(container.querySelectorAll("button, a, [tabindex]")).toHaveLength(0);
  });

  it("shows its caption as its heading when shown alone", () => {
    drawVisual(FIXTURE);
    expect(
      screen.getByRole("heading", {
        name: "How plates, density and subduction connect",
      }),
    ).toBeVisible();
  });

  it("leaves the caption to the frame around it", () => {
    const { container } = drawVisual(FIXTURE, { framed: true });
    expect(screen.queryByRole("heading")).toBeNull();
    expect(container).not.toHaveTextContent(
      "How plates, density and subduction connect",
    );
  });

  describe.each(schemes)("in the %s theme", (scheme) => {
    it("draws in the text colour on panel and canvas faces", async () => {
      const { svg } = drawVisual(TWELVE_NODE_MAPS.web);
      await applyStyles(scheme);
      expectFigurePaints(svg);
      const face = groupTitled(svg, "Trench").querySelector("rect")!;
      const paint = getComputedStyle(face);
      expect(paint.stroke).toBe("currentcolor");
      expect(paint.fill).not.toBe("none");
    });
  });
});
