import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { CatalogVisual } from "@/catalog/catalog-visual";
import {
  applyStyles,
  clearStyles,
  literalColorsIn,
  paintOf,
  schemes,
  tokenOf,
} from "@/test/styles";

afterEach(clearStyles);

/** The callout the core's tests validate. */
const fixture: unknown = JSON.parse(
  readFileSync(
    resolve(
      import.meta.dirname,
      "../../../../../crates/atlas-core/tests/fixtures/catalog/valid/callout.json",
    ),
    "utf8",
  ),
);

const TITLE = "Convergent does not mean a crash";
const BODY =
  "Nothing collides at speed: plates move about as fast as fingernails grow.";

function callout(title: string, body: string): unknown {
  return {
    root: "note",
    elements: {
      note: { type: "callout", props: { title, body }, children: [] },
    },
  };
}

describe.each(schemes)("a callout in the %s theme", (scheme) => {
  it("is a note named by its title, with the title as a heading and the body after it", async () => {
    render(<CatalogVisual spec={fixture} />);
    await applyStyles(scheme);

    const note = screen.getByRole("note", { name: TITLE });
    const heading = within(note).getByRole("heading", { name: TITLE });
    expect(note).toHaveTextContent(`${TITLE}${BODY}`);
    expect(note.textContent?.indexOf(BODY)).toBeGreaterThan(
      note.textContent?.indexOf(TITLE) ?? -1,
    );
    expect(getComputedStyle(heading).fontWeight).toBe("510");
    expect(tokenOf(paintOf(heading).color, heading)).toBe("heading");
    const body = within(note).getByText(BODY);
    expect(getComputedStyle(body).fontWeight).not.toBe("510");
    expect(tokenOf(paintOf(body).color, body)).toBe("body");
  });

  it("is a hairline box with 12 px corners: no fill, no shadow, no icon", async () => {
    render(<CatalogVisual spec={fixture} />);
    await applyStyles(scheme);

    const note = screen.getByRole("note", { name: TITLE });
    const style = getComputedStyle(note);
    expect(style.borderTopWidth).toBe("1px");
    expect(style.borderTopStyle).toBe("solid");
    expect(tokenOf(paintOf(note).border ?? "", note)).toBe("edge");
    expect(style.borderTopLeftRadius).toBe("12px");
    for (const element of [note, ...note.querySelectorAll("*")]) {
      expect(tokenOf(paintOf(element).background, element)).toBe("transparent");
    }
    expect(["none", ""]).toContain(
      style.boxShadow.replaceAll(/rgba\(0, 0, 0, 0\) 0px 0px(, )?/g, ""),
    );
    expect(note.querySelector("svg, img")).toBeNull();
    expect(within(note).queryByRole("img")).toBeNull();
    expect(literalColorsIn(note)).toEqual([]);
  });

  it("wraps a long title and body inside its box", async () => {
    const title =
      "Subduction-zone-megathrust-earthquakes-are-the-largest-on-Earth and they repeat";
    const body =
      "A locked plate boundary stores strain for centuries before it slips. ".repeat(
        6,
      );
    render(<CatalogVisual spec={callout(title, body.trim())} />);
    await applyStyles(scheme);

    const note = screen.getByRole("note", { name: title });
    expect(note).toHaveTextContent(body.trim());
    for (const element of [note, ...note.querySelectorAll("*")]) {
      const style = getComputedStyle(element);
      expect(["break-word", "anywhere"]).toContain(style.overflowWrap);
      expect(style.whiteSpace).not.toBe("nowrap");
      expect(style.overflowX).not.toBe("hidden");
    }
  });

  it("keeps its text selectable", async () => {
    render(<CatalogVisual spec={fixture} />);
    await applyStyles(scheme);

    const note = screen.getByRole("note", { name: TITLE });
    for (const element of [note, ...note.querySelectorAll("*")]) {
      expect(getComputedStyle(element).userSelect).not.toBe("none");
    }
  });
});
