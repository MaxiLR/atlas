import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { render, screen, within } from "@testing-library/react";
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

/** The timeline the core's tests validate. */
const fixture: unknown = JSON.parse(
  readFileSync(
    resolve(
      import.meta.dirname,
      "../../../../../crates/atlas-core/tests/fixtures/catalog/valid/timeline.json",
    ),
    "utf8",
  ),
);

const CAPTION = "How plate tectonics was accepted";

type Event = { date: string; label: string; note?: string };

function timeline(caption: string, events: Event[]): unknown {
  return {
    root: "history",
    elements: {
      history: { type: "timeline", props: { caption, events }, children: [] },
    },
  };
}

/** A timeline as the section view shows it: inside the figure frame. */
function renderFramed(spec: unknown, caption: string) {
  render(
    <Figure label="Fig. 2" caption={caption}>
      <CaptionInFrame value={true}>
        <CatalogVisual spec={spec} />
      </CaptionInFrame>
    </Figure>,
  );
  return screen.getByRole("figure");
}

/** The drawn marks of an event: hidden from assistive technology. */
function marksOf(item: HTMLElement): Element[] {
  return [...item.querySelectorAll('[aria-hidden="true"]')];
}

describe.each(schemes)("a timeline in the %s theme", (scheme) => {
  it("is a list of events named by its caption, each date, label and note in order", async () => {
    const figure = renderFramed(fixture, CAPTION);
    await applyStyles(scheme);

    const list = within(figure).getByRole("list", { name: CAPTION });
    const items = within(list).getAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual([
      "1912Wegener proposes continental drift",
      "1960sSea-floor spreading is mapped" +
        "Magnetic stripes either side of the ridges",
    ]);
    expect(within(figure).queryByRole("heading")).toBeNull();
    expect(literalColorsIn(figure)).toEqual([]);
  });

  it("shows its caption as a heading on its own", async () => {
    render(<CatalogVisual spec={fixture} />);
    await applyStyles(scheme);

    expect(screen.getByRole("heading", { name: CAPTION })).toBeVisible();
    expect(screen.getByRole("list", { name: CAPTION })).toBeVisible();
  });

  it("labels each event with its date in uppercase mono in the muted colour", async () => {
    const figure = renderFramed(fixture, CAPTION);
    await applyStyles(scheme);

    for (const date of ["1912", "1960s"]) {
      const label = within(figure).getByText(date);
      const style = getComputedStyle(label);
      expect(style.fontFamily).toMatch(/^"?IBM Plex Mono/);
      expect(style.fontSize).toBe("12px");
      expect(style.textTransform).toBe("uppercase");
      expect(tokenOf(paintOf(label).color, label)).toBe("muted");
    }
    const label = within(figure).getByText("Sea-floor spreading is mapped");
    expect(tokenOf(paintOf(label).color, label)).toBe("heading");
  });

  it("draws the events along a line: a hollow node on each, rules between them", async () => {
    const figure = renderFramed(fixture, CAPTION);
    await applyStyles(scheme);

    const items = within(figure).getAllByRole("listitem");
    for (const item of items) {
      const nodes = marksOf(item).filter((mark) => {
        const style = getComputedStyle(mark);
        return (
          style.borderTopWidth === "1px" &&
          style.borderTopStyle === "solid" &&
          style.borderTopLeftRadius !== "0px"
        );
      });
      expect(nodes).toHaveLength(1);
      expect(tokenOf(paintOf(nodes[0]).background, nodes[0])).toBe(
        "transparent",
      );
    }
    // The line joins the first event to the second: drawn below the first
    // node and above the second, in a rule token.
    const rules = items.map((item) =>
      marksOf(item).filter((mark) => {
        const style = getComputedStyle(mark);
        return (
          style.width === "1px" &&
          tokenOf(paintOf(mark).background, mark) !== "transparent"
        );
      }),
    );
    expect(rules[0].length).toBeGreaterThan(0);
    expect(rules[1].length).toBeGreaterThan(0);
    for (const rule of rules.flat()) {
      expect(["faint", "edge", "edge-strong", "muted"]).toContain(
        tokenOf(paintOf(rule).background, rule),
      );
    }
    expect(literalColorsIn(figure)).toEqual([]);
  });

  it("draws a minimal timeline of two events, without notes", async () => {
    const figure = renderFramed(
      timeline("Two dates", [
        { date: "1915", label: "Drift published" },
        { date: "1968", label: "Plate tectonics named" },
      ]),
      "Two dates",
    );
    await applyStyles(scheme);

    const items = within(
      within(figure).getByRole("list", { name: "Two dates" }),
    ).getAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual([
      "1915Drift published",
      "1968Plate tectonics named",
    ]);
    expect(literalColorsIn(figure)).toEqual([]);
  });

  it("stacks a long timeline vertically and wraps long dates and text", async () => {
    const events = Array.from({ length: 24 }, (_, n) => ({
      date:
        n === 0
          ? "Late-Cretaceous-about-seventy-million-years-ago"
          : `${1900 + n * 4}`,
      label: `Event ${n + 1}: ${"the ridge spreads and the slab sinks ".repeat(3).trim()}`,
      note:
        n % 2 === 0
          ? "Measured from magnetic anomalies recorded on both flanks of the ridge."
          : undefined,
    }));
    const figure = renderFramed(timeline("Many events", events), "Many events");
    await applyStyles(scheme);

    const list = within(figure).getByRole("list", { name: "Many events" });
    const items = within(list).getAllByRole("listitem");
    expect(items).toHaveLength(24);
    expect(items[0]).toHaveTextContent(events[0].date);
    expect(items[23]).toHaveTextContent("Event 24:");

    const listStyle = getComputedStyle(list);
    expect(
      listStyle.flexDirection === "row" && listStyle.display.includes("flex"),
    ).toBe(false);
    expect(listStyle.whiteSpace).not.toBe("nowrap");
    expect(listStyle.height === "" || listStyle.height === "auto").toBe(true);
    for (const item of items) {
      for (const element of [item, ...item.querySelectorAll("*")]) {
        if (element.getAttribute("aria-hidden") === "true") continue;
        const style = getComputedStyle(element);
        expect(style.whiteSpace).not.toBe("nowrap");
        expect(style.textOverflow).not.toBe("ellipsis");
      }
    }
    const longDate = within(items[0]).getByText(events[0].date);
    expect(["break-word", "anywhere"]).toContain(
      getComputedStyle(longDate).overflowWrap,
    );
  });

  it("keeps its text selectable", async () => {
    const figure = renderFramed(fixture, CAPTION);
    await applyStyles(scheme);

    for (const element of within(figure)
      .getByRole("list")
      .querySelectorAll("*")) {
      expect(getComputedStyle(element).userSelect).not.toBe("none");
    }
  });
});
