import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { catalog } from "./catalog.ts";
import { CatalogVisual } from "./catalog-visual.tsx";
import { registry } from "./registry.ts";

/** The specs the core's tests validate: one valid spec per entry. */
const FIXTURES = resolve(
  import.meta.dirname,
  "../../../crates/atlas-core/tests/fixtures/catalog",
);

function fixture(kind: "valid" | "invalid", name: string): unknown {
  return JSON.parse(
    readFileSync(resolve(FIXTURES, kind, `${name}.json`), "utf8"),
  );
}

/** The catalog's entries, as the ticket names them. */
const ENTRIES = [
  "concept-map",
  "layered-figure",
  "timeline",
  "step-through",
  "comparison",
  "callout",
  "mermaid-diagram",
  "figure",
  "check-question",
  "citation",
];

/**
 * What each valid fixture must show as text: its heading (none for the
 * citation, which sits inside a sentence) and its content, in order.
 */
const SHOWN: Record<string, { heading?: string; text: string[] }> = {
  "concept-map": {
    heading: "How plates, density and subduction connect",
    text: [
      "Oceanic plate",
      "Continental plate",
      "Trench",
      "sinks beneath",
      "bends into",
    ],
  },
  "layered-figure": {
    heading: "The layers under a continent",
    text: [
      "Crust",
      "Thin, cold and brittle",
      "Lithospheric mantle",
      "Rigid, moves with the crust",
      "Asthenosphere",
      "Hot enough to flow slowly",
    ],
  },
  timeline: {
    heading: "How plate tectonics was accepted",
    text: [
      "1912",
      "Wegener proposes continental drift",
      "1960s",
      "Sea-floor spreading is mapped",
      "Magnetic stripes either side of the ridges",
    ],
  },
  "step-through": {
    heading: "How a volcanic arc forms",
    text: [
      "The slab sinks",
      "The denser plate descends into the mantle",
      "Water is released",
      "Heat drives water out of the slab",
      "Rock melts",
      "Water lowers the melting point of the mantle above",
    ],
  },
  comparison: {
    heading: "The same collision, three outcomes",
    text: [
      "Ocean meets continent",
      "Ocean meets ocean",
      "Continent meets continent",
      "What sinks",
      "The oceanic plate",
      "The older, colder plate",
      "Neither, for long",
      "What forms",
      "A trench and a volcanic arc",
      "An island arc",
      "A high mountain belt",
    ],
  },
  callout: {
    heading: "Convergent does not mean a crash",
    text: [
      "Nothing collides at speed: plates move about as fast as fingernails grow.",
    ],
  },
  "mermaid-diagram": {
    heading: "Where the sinking plate goes",
    text: ["flowchart LR", "A[Oceanic plate] --> B[Trench]"],
  },
  figure: {
    heading: "Sea level, not to scale",
    text: [],
  },
  "check-question": {
    heading: "A young, warm oceanic plate meets an old, cold one. Which sinks?",
    text: [
      "The old, cold plate",
      "The young, warm plate",
      "Neither: they build mountains",
      "They slide past each other",
    ],
  },
  citation: {
    text: ["1", "Understanding plate motions"],
  },
};

/** Each component module, by the folder it sits in. */
const FOLDERS = Object.keys(
  import.meta.glob(["./components/*/*.tsx", "!./components/**/*.test.tsx"]),
).map((path) => {
  const [, , folder, file] = path.split("/");
  return { folder, file };
});

describe("the catalog and its registry", () => {
  it("has exactly the ticket's entries", () => {
    expect([...catalog.componentNames].sort()).toEqual([...ENTRIES].sort());
  });

  it("maps every catalog name to a registry component and the reverse", () => {
    expect(Object.keys(registry).sort()).toEqual(
      [...catalog.componentNames].sort(),
    );
  });

  it("keeps each component in a folder named after its entry", () => {
    expect(FOLDERS.map(({ folder }) => folder).sort()).toEqual(
      [...catalog.componentNames].sort(),
    );
    for (const { folder, file } of FOLDERS) {
      expect(file).toBe(`${folder}.tsx`);
    }
  });

  it("has one valid fixture per entry", () => {
    expect(
      readdirSync(resolve(FIXTURES, "valid"))
        .map((file) => file.replace(/\.json$/, ""))
        .sort(),
    ).toEqual([...ENTRIES].sort());
  });
});

describe("CatalogVisual", () => {
  it.each(ENTRIES)("renders the %s fixture's content as text", (name) => {
    const onInvalid = vi.fn();
    const { container } = render(
      <CatalogVisual spec={fixture("valid", name)} onInvalid={onInvalid} />,
    );
    const { heading, text } = SHOWN[name];

    if (heading !== undefined) {
      expect(screen.getByRole("heading", { name: heading })).toBeVisible();
    }
    const shown = container.textContent ?? "";
    let from = 0;
    for (const part of text) {
      const at = shown.indexOf(part, from);
      expect(at, `"${part}" after offset ${from} in "${shown}"`).not.toBe(-1);
      from = at + part.length;
    }
    expect(onInvalid).not.toHaveBeenCalled();
  });

  it("shows a check question's options without telling the answer", () => {
    const { container } = render(
      <CatalogVisual spec={fixture("valid", "check-question")} />,
    );
    expect(container).not.toHaveTextContent(
      "Oceanic lithosphere grows denser as it ages",
    );
    expect(container).not.toHaveTextContent("Thinks warmth makes a plate");
  });

  it("never inserts a figure's SVG in its plain form", () => {
    const { container } = render(
      <CatalogVisual spec={fixture("valid", "figure")} />,
    );
    expect(container.querySelector("svg")).toBeNull();
    expect(container).not.toHaveTextContent("<svg");
  });

  it("renders nothing for an unknown component and reports why", () => {
    const onInvalid = vi.fn();
    const { container } = render(
      <CatalogVisual
        spec={fixture("invalid", "unknown-component")}
        onInvalid={onInvalid}
      />,
    );
    expect(container).toBeEmptyDOMElement();
    expect(onInvalid).toHaveBeenCalledTimes(1);
    expect(onInvalid.mock.calls[0][0].join("\n")).toContain("bar-chart");
  });

  it.each(["missing-prop", "wrong-type"])(
    "renders nothing for the %s fixture and reports why",
    (name) => {
      const onInvalid = vi.fn();
      const { container } = render(
        <CatalogVisual spec={fixture("invalid", name)} onInvalid={onInvalid} />,
      );
      expect(container).toBeEmptyDOMElement();
      expect(onInvalid).toHaveBeenCalledTimes(1);
      expect(onInvalid.mock.calls[0][0].length).toBeGreaterThan(0);
    },
  );

  it("renders nothing for a spec that is not an object", () => {
    const onInvalid = vi.fn();
    const { container } = render(
      <CatalogVisual spec="<script>alert(1)</script>" onInvalid={onInvalid} />,
    );
    expect(container).toBeEmptyDOMElement();
    expect(onInvalid).toHaveBeenCalledTimes(1);
  });
});
