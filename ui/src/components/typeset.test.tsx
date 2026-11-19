import { render, screen } from "@testing-library/react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { afterEach, describe, expect, it } from "vitest";

import {
  applyStyles,
  clearStyles,
  literalColorsIn,
  paintOf,
  sameColor,
  schemes,
  token,
  tokenOf,
} from "@/test/styles";

afterEach(clearStyles);

/** A lesson in Markdown, as a section streams it, block by block. */
const sample = [
  "Plates move toward each other at a few centimetres a year. Where two of them converge, one plate bends and slides beneath the other, a process called **subduction**.",
  "## Three kinds of convergence",
  "What you get at the surface depends on what the two plates carry:",
  "- Ocean meets continent: a trench and a volcanic arc\n- Ocean meets ocean: an arc of islands\n- Continent meets continent: a mountain belt",
  "1. The slab sinks.\n2. Water lowers the melting point of the rock above.",
  "| Boundary | What sinks |\n| --- | --- |\n| Ocean and continent | The oceanic plate |\n| Continent and continent | Neither |",
  "The distance is `d = v t`; the [USGS](https://www.usgs.gov) explains it.",
  "```\nd = 7 cm/yr × 10^6 yr = 70 km\n```",
  "> Convergent does not mean a crash.",
];

function Prose({ markdown }: { markdown: string }) {
  return (
    <div className="typeset" data-testid="prose">
      <Markdown remarkPlugins={[remarkGfm]}>{markdown}</Markdown>
    </div>
  );
}

/** Everything about an element's box that moves what follows it. */
function layoutOf(element: Element) {
  const style = getComputedStyle(element);
  return [
    "display",
    "position",
    "margin-top",
    "margin-bottom",
    "margin-left",
    "margin-right",
    "padding-top",
    "padding-bottom",
    "padding-left",
    "padding-right",
    "border-top-width",
    "border-bottom-width",
    "font-size",
    "line-height",
    "letter-spacing",
    "font-weight",
    "white-space",
  ].map((property) => `${property}: ${style.getPropertyValue(property)}`);
}

const layoutOfAll = (elements: Element[]) => elements.map(layoutOf);

describe.each(schemes)("in the %s theme", (scheme) => {
  it("sets lesson prose at 16 px / 1.65 with -0.011em tracking in the body colour", async () => {
    render(<Prose markdown={sample.join("\n\n")} />);
    await applyStyles(scheme);

    const paragraph = screen.getByText(/Plates move toward each other/);
    const style = getComputedStyle(paragraph);
    expect(style.fontSize).toBe("16px");
    expect(parseFloat(style.lineHeight)).toBeCloseTo(26.4, 1);
    expect(parseFloat(style.letterSpacing)).toBeCloseTo(-0.176, 3);
    expect(sameColor(style.color, token(paragraph, "body"))).toBe(true);
  });

  it("sets headings at weight 510 and section headings at 20 px in the heading colour", async () => {
    render(<Prose markdown={sample.join("\n\n")} />);
    await applyStyles(scheme);

    const heading = screen.getByRole("heading", {
      name: "Three kinds of convergence",
    });
    const style = getComputedStyle(heading);
    expect(style.fontSize).toBe("20px");
    expect(style.fontWeight).toBe("510");
    expect(sameColor(style.color, token(heading, "heading"))).toBe(true);
  });

  it("draws links, lists, code and tables with tokens only", async () => {
    render(<Prose markdown={sample.join("\n\n")} />);
    await applyStyles(scheme);

    const prose = screen.getByTestId("prose");
    expect(literalColorsIn(prose)).toEqual([]);
    const link = screen.getByRole("link", { name: "USGS" });
    expect(tokenOf(paintOf(link).color, link)).toBeDefined();
    expect(getComputedStyle(link).textDecorationLine).toBe("underline");
    expect(screen.getAllByRole("listitem")).toHaveLength(5);
    expect(screen.getByRole("table")).toBeVisible();
    const code = screen.getByText("d = v t");
    expect(tokenOf(paintOf(code).background, code)).not.toBe("transparent");
    expect(getComputedStyle(code).fontFamily).toMatch(/^"?IBM Plex Mono/);
  });

  it("keeps earlier blocks where they are when a later block is appended or streams", async () => {
    const { rerender } = render(<Prose markdown={sample.join("\n\n")} />);
    await applyStyles(scheme);
    const prose = screen.getByTestId("prose");
    const earlier = [...prose.querySelectorAll("*")];
    const before = layoutOfAll(earlier);

    const streamed =
      "A plate moving at 7 cm a year covers 70 km in a million years.";
    for (const later of [
      "## How slow",
      "## How slow is slow",
      `## How slow is slow\n\n${streamed.slice(0, 20)}`,
      `## How slow is slow\n\n${streamed}`,
      `## How slow is slow\n\n${streamed}\n\n- one\n- two`,
    ]) {
      rerender(<Prose markdown={[...sample, later].join("\n\n")} />);
      await applyStyles(scheme);
      expect(earlier.every((element) => element.isConnected)).toBe(true);
      expect(layoutOfAll(earlier)).toEqual(before);
    }
  });
});
