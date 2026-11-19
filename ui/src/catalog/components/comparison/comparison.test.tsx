import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { CatalogVisual } from "@/catalog/catalog-visual";
import { CaptionInFrame } from "@/catalog/frame-context";
import { Figure } from "@/components/ui/figure";
import {
  applyStyles,
  clearStyles,
  literalColorsIn,
  paintOf,
  schemes,
  tokenOf,
} from "@/test/styles";

afterEach(clearStyles);

/** The comparison the core's tests validate. */
const fixture: unknown = JSON.parse(
  readFileSync(
    resolve(
      import.meta.dirname,
      "../../../../../crates/atlas-core/tests/fixtures/catalog/valid/comparison.json",
    ),
    "utf8",
  ),
);

const CAPTION = "The same collision, three outcomes";

function comparison(props: {
  caption: string;
  columns: string[];
  rows: { label: string; cells: string[] }[];
}): unknown {
  return {
    root: "table",
    elements: { table: { type: "comparison", props, children: [] } },
  };
}

/** A comparison as the section view shows it: inside the figure frame. */
function renderFramed(spec: unknown, caption: string) {
  render(
    <Figure label="Table 1" caption={caption}>
      <CaptionInFrame value={true}>
        <CatalogVisual spec={spec} />
      </CaptionInFrame>
    </Figure>,
  );
  return screen.getByRole("figure");
}

function texts(elements: HTMLElement[]): string[] {
  return elements.map((element) => element.textContent ?? "");
}

/** The nearest element between `element` and `stop` that scrolls sideways. */
function scrollerOf(element: Element, stop: Element): Element | null {
  for (
    let node = element.parentElement;
    node !== null && node !== stop;
    node = node.parentElement
  ) {
    if (["auto", "scroll"].includes(getComputedStyle(node).overflowX)) {
      return node;
    }
  }
  return null;
}

describe.each(schemes)("a comparison in the %s theme", (scheme) => {
  it("is a table named by its caption, with a header row and a label column in order", async () => {
    render(<CatalogVisual spec={fixture} />);
    await applyStyles(scheme);

    const table = screen.getByRole("table", { name: CAPTION });
    const [header, ...body] = within(table).getAllByRole("row");
    expect(texts(within(header).getAllByRole("columnheader"))).toEqual([
      "Ocean meets continent",
      "Ocean meets ocean",
      "Continent meets continent",
    ]);
    expect(
      body.map((row) => [
        ...texts(within(row).getAllByRole("rowheader")),
        ...texts(within(row).getAllByRole("cell")),
      ]),
    ).toEqual([
      [
        "What sinks",
        "The oceanic plate",
        "The older, colder plate",
        "Neither, for long",
      ],
      [
        "What forms",
        "A trench and a volcanic arc",
        "An island arc",
        "A high mountain belt",
      ],
    ]);
  });

  it("shows its caption as a heading on its own, and its content only inside a frame", async () => {
    render(<CatalogVisual spec={fixture} />);
    expect(screen.getByRole("heading", { name: CAPTION })).toBeVisible();

    cleanup();
    const figure = renderFramed(fixture, CAPTION);
    await applyStyles(scheme);

    expect(within(figure).queryByRole("heading")).toBeNull();
    expect(
      within(figure).getByRole("table", { name: CAPTION }),
    ).toBeInTheDocument();
    expect(figure.textContent?.split(CAPTION).length).toBe(2);
  });

  it("separates its cells with hairlines, not filled tiles, with heads in the heading colour at 510", async () => {
    const figure = renderFramed(fixture, CAPTION);
    await applyStyles(scheme);

    const table = within(figure).getByRole("table");
    const [header, ...body] = within(table).getAllByRole("row");

    for (const cell of table.querySelectorAll("th, td")) {
      expect(tokenOf(paintOf(cell).background, cell)).toBe("transparent");
    }
    for (const head of within(header).getAllByRole("columnheader")) {
      const style = getComputedStyle(head);
      expect(style.fontWeight).toBe("510");
      expect(tokenOf(paintOf(head).color, head)).toBe("heading");
      expect(style.borderLeftWidth).toBe("1px");
      expect(tokenOf(style.borderLeftColor, head)).toBe("edge");
    }
    for (const row of body) {
      const label = within(row).getByRole("rowheader");
      expect(tokenOf(paintOf(label).color, label)).toBe("muted");
      for (const cell of [label, ...within(row).getAllByRole("cell")]) {
        const style = getComputedStyle(cell);
        expect(style.borderTopWidth).toBe("1px");
        expect(tokenOf(style.borderTopColor, cell)).toBe("edge");
      }
      for (const cell of within(row).getAllByRole("cell")) {
        const style = getComputedStyle(cell);
        expect(style.borderLeftWidth).toBe("1px");
        expect(tokenOf(style.borderLeftColor, cell)).toBe("edge");
      }
    }
    expect(literalColorsIn(figure)).toEqual([]);
  });

  it("keeps a minimal comparison of two columns and one row to the frame's width", async () => {
    const figure = renderFramed(
      comparison({
        caption: "Two plates",
        columns: ["Oceanic", "Continental"],
        rows: [{ label: "Density", cells: ["Higher", "Lower"] }],
      }),
      "Two plates",
    );
    await applyStyles(scheme);

    const table = within(figure).getByRole("table", { name: "Two plates" });
    expect(texts(within(table).getAllByRole("columnheader"))).toEqual([
      "Oceanic",
      "Continental",
    ]);
    expect(texts(within(table).getAllByRole("rowheader"))).toEqual(["Density"]);
    expect(texts(within(table).getAllByRole("cell"))).toEqual([
      "Higher",
      "Lower",
    ]);
    const style = getComputedStyle(table);
    expect(style.width).toBe("100%");
    // Fits the narrowest reading column (a 720 px window) without scrolling.
    expect(parseFloat(style.minWidth || "0")).toBeLessThanOrEqual(600);
    expect(literalColorsIn(figure)).toEqual([]);
  });

  it("scrolls many columns inside the frame instead of squeezing them", async () => {
    const columns = Array.from({ length: 8 }, (_, n) => `Plate ${n + 1}`);
    const rows = Array.from({ length: 6 }, (_, r) => ({
      label: `Aspect ${r + 1}`,
      cells: columns.map((_, c) => `Value ${r + 1}.${c + 1}`),
    }));
    const figure = renderFramed(
      comparison({ caption: "Eight plates", columns, rows }),
      "Eight plates",
    );
    await applyStyles(scheme);

    const table = within(figure).getByRole("table", { name: "Eight plates" });
    expect(texts(within(table).getAllByRole("columnheader"))).toEqual(columns);
    expect(within(table).getAllByRole("row")).toHaveLength(7);
    expect(within(table).getAllByRole("cell")).toHaveLength(48);

    // Every compared column keeps at least 8rem, so eight of them are wider
    // than the reading column and scroll sideways inside the frame.
    expect(
      parseFloat(getComputedStyle(table).minWidth || "0"),
    ).toBeGreaterThanOrEqual(8 * 128);
    expect(scrollerOf(table, figure)).not.toBeNull();
    expect(getComputedStyle(figure).overflowX).not.toBe("visible");
  });

  it("wraps long content in its cells rather than widening or clipping them", async () => {
    const long =
      "Lithosphere-asthenosphere-boundary-depth-varies-with-plate-age-and-temperature, and the slab keeps sinking for hundreds of kilometres before it stalls.";
    const figure = renderFramed(
      comparison({
        caption: "Long cells",
        columns: ["Young plate", long],
        rows: [{ label: long, cells: [long, "Short"] }],
      }),
      "Long cells",
    );
    await applyStyles(scheme);

    const table = within(figure).getByRole("table", { name: "Long cells" });
    for (const cell of table.querySelectorAll("th, td")) {
      const style = getComputedStyle(cell);
      expect(["break-word", "anywhere"]).toContain(style.overflowWrap);
      expect(style.whiteSpace).not.toBe("nowrap");
      expect(style.textOverflow).not.toBe("ellipsis");
    }
    expect(table).toHaveTextContent(long);
  });

  it("keeps its text selectable", async () => {
    const figure = renderFramed(fixture, CAPTION);
    await applyStyles(scheme);

    for (const element of figure.querySelectorAll("table, th, td")) {
      expect(getComputedStyle(element).userSelect).not.toBe("none");
    }
  });
});
