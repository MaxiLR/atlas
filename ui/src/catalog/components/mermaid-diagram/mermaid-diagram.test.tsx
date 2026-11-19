import { render, screen, waitFor, within } from "@testing-library/react";
import { converter, differenceEuclidean } from "culori";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { CatalogVisual } from "@/catalog/catalog-visual";
import { CaptionInFrame } from "@/catalog/frame-context";
import { Figure } from "@/components/ui/figure";
import { applyStyles, clearStyles, schemes, tokensAt } from "@/test/styles";

/**
 * jsdom lays nothing out, and Mermaid measures its labels while it draws.
 * These stand in for the browser's measurements: about 7 px a character,
 * 14 px a line.
 */
beforeAll(() => {
  const measured = window.SVGElement.prototype as unknown as {
    getBBox(): DOMRect;
    getComputedTextLength(): number;
  };
  measured.getBBox = function (this: SVGElement) {
    const length = (this.textContent ?? "").length;
    return new DOMRect(0, 0, length * 7, 14);
  };
  measured.getComputedTextLength = function (this: SVGElement) {
    return (this.textContent ?? "").length * 7;
  };
});

/** Mermaid loads on first use; the first diagram takes longest. */
vi.setConfig({ testTimeout: 15_000 });

afterEach(() => {
  clearStyles();
  vi.restoreAllMocks();
});

const CAPTION = "Where the sinking plate goes";
const FLOWCHART =
  "flowchart LR\n  A[Oceanic plate] --> B[Trench]\n  B --> C[Mantle]";

function diagram(source: string, caption = CAPTION): unknown {
  return {
    root: "flow",
    elements: { flow: { type: "mermaid-diagram", props: { source, caption } } },
  };
}

/** A diagram as the section view shows it: inside the figure frame. */
function renderFramed(source: string) {
  render(
    <Figure label="Fig. 2" caption={CAPTION}>
      <CaptionInFrame value={true}>
        <CatalogVisual spec={diagram(source)} />
      </CaptionInFrame>
    </Figure>,
  );
  return screen.getByRole("figure");
}

/** The drawn diagram, once Mermaid has rendered it. */
async function drawn(frame: HTMLElement): Promise<SVGSVGElement> {
  const svg = await within(frame).findByRole(
    "img",
    { name: CAPTION },
    { timeout: 10_000 },
  );
  expect(svg.namespaceURI).toBe("http://www.w3.org/2000/svg");
  return svg as unknown as SVGSVGElement;
}

const distance = differenceEuclidean("oklab");
const toOklab = converter("oklab");

/**
 * The theme token `color` is, where `element` is. Mermaid is given the
 * tokens as values and jsdom rounds them to whole sRGB steps, so the match
 * allows that rounding and nothing coarser.
 */
function tokenNear(color: string, element: Element): string | undefined {
  const parsed = toOklab(color);
  if (!parsed || (parsed.alpha ?? 1) === 0) return undefined;
  let best: { name: string; gap: number } | undefined;
  for (const [name, value] of tokensAt(element)) {
    const candidate = toOklab(value);
    if (!candidate || (candidate.alpha ?? 1) < 1) continue;
    const gap = distance(parsed, candidate);
    if (gap < 0.005 && (best === undefined || gap < best.gap)) {
      best = { name, gap };
    }
  }
  return best?.name;
}

/** Every fill and stroke the diagram paints with, named by token. */
function paints(svg: Element): Set<string> {
  const named = new Set<string>();
  for (const element of svg.querySelectorAll(
    "path, rect, circle, ellipse, line, polygon, polyline, text, tspan",
  )) {
    if (element.closest("marker, defs") !== null) continue;
    const style = getComputedStyle(element);
    for (const value of [style.fill, style.stroke]) {
      if (["none", "", "transparent"].includes(value)) continue;
      const resolved =
        value.toLowerCase() === "currentcolor" ? style.color : value;
      named.add(tokenNear(resolved, element) ?? `literal ${resolved}`);
    }
  }
  return named;
}

/** The theme token a label is painted with. */
function labelPaint(svg: Element, word: string): string | undefined {
  const label = within(svg as unknown as HTMLElement).getByText(word);
  return tokenNear(getComputedStyle(label).fill, label);
}

describe.each(schemes)("a Mermaid diagram in the %s theme", (scheme) => {
  it("draws a valid flowchart's nodes and labels in the theme's colours, with no request", async () => {
    const fetch = vi.spyOn(globalThis, "fetch");
    await applyStyles(scheme);
    const frame = renderFramed(FLOWCHART);
    const svg = await drawn(frame);
    await applyStyles(scheme);

    expect(svg).toHaveTextContent(/Oceanic\s*plate/);
    expect(svg).toHaveTextContent("Trench");
    expect(svg).toHaveTextContent("Mantle");
    expect(frame).not.toHaveTextContent("flowchart LR");

    expect(labelPaint(svg, "Trench")).toBe("muted");
    const painted = paints(svg);
    expect([...painted].filter((name) => name.startsWith("literal"))).toEqual(
      [],
    );
    expect(painted).toContain("body");
    expect(painted).toContain("raised");
    expect(fetch).not.toHaveBeenCalled();
  });

  it("re-renders in the other theme's colours when the theme changes", async () => {
    await applyStyles(scheme);
    const frame = renderFramed(FLOWCHART);
    await drawn(frame);
    const labelColor = () =>
      getComputedStyle(within(frame).getByText("Trench")).fill;
    const before = labelColor();
    expect(tokenNear(before, frame)).toBe("muted");

    const other = scheme === "light" ? "dark" : "light";
    await applyStyles(other);

    await waitFor(
      () => expect(distance(labelColor(), before)).toBeGreaterThan(0.05),
      { timeout: 10_000 },
    );
    const svg = await drawn(frame);
    expect(labelPaint(svg, "Trench")).toBe("muted");
    const painted = paints(svg);
    expect([...painted].filter((name) => name.startsWith("literal"))).toEqual(
      [],
    );
    expect(painted).toContain("body");
    expect(painted).toContain("raised");
  });
});

/** Every attribute in the subtree, as `element@name=value`. */
function attributesIn(root: Element): string[] {
  return [root, ...root.querySelectorAll("*")].flatMap((element) =>
    [...element.attributes].map(
      (attribute) =>
        `${element.localName}@${attribute.name}=${attribute.value}`,
    ),
  );
}

describe("a Mermaid diagram that tries to act", () => {
  it("draws a click directive and HTML labels with no handler, link or HTML element", async () => {
    const callback = vi.fn();
    Object.assign(window, { callback });
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    await applyStyles("dark");
    const frame = renderFramed(
      [
        '%%{init: {"securityLevel": "loose", "htmlLabels": true, "flowchart": {"htmlLabels": true}}}%%',
        "flowchart LR",
        '  A["<b>Trench</b> <img src=x onerror=callback()>"] --> B["<a href=\'https://evil.example/\'>Arc</a>"]',
        '  C["<i>Melt</i>"] --> D[Volcano]',
        '  click A "https://evil.example/" "Open"',
        '  click B callback "Run"',
        "  click C call callback()",
        '  click D href "javascript:callback()"',
      ].join("\n"),
    );
    const svg = await drawn(frame);
    expect(svg).toHaveTextContent("Volcano");

    for (const name of ["a", "foreignObject", "img", "b", "i", "script"]) {
      expect(
        frame.getElementsByTagName(name),
        `<${name}> in the diagram`,
      ).toHaveLength(0);
    }
    expect(
      [...svg.querySelectorAll("*")].filter(
        (element) => element.namespaceURI !== svg.namespaceURI,
      ),
    ).toEqual([]);
    const attributes = attributesIn(svg);
    expect(attributes.filter((entry) => /@on/i.test(entry))).toEqual([]);
    expect(
      attributes.filter((entry) => /@(xlink:)?href=/i.test(entry)),
    ).toEqual([]);
    expect(attributes.filter((entry) => /javascript:/i.test(entry))).toEqual(
      [],
    );

    for (const element of svg.querySelectorAll("*")) {
      element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    }
    expect(callback).not.toHaveBeenCalled();
    expect(open).not.toHaveBeenCalled();
  });

  it("shows the description for a diagram Mermaid cannot parse, never an error", async () => {
    await applyStyles("dark");
    const frame = renderFramed("flowchart LR\n  A[Trench] -->");

    expect(
      await within(frame).findByText(
        /could not be drawn/,
        {},
        { timeout: 10_000 },
      ),
    ).toBeVisible();
    expect(within(frame).getByText(CAPTION)).toBeVisible();
    expect(frame.querySelector("svg")).toBeNull();
    expect(document.body).not.toHaveTextContent(
      /syntax|parse|error|expecting|line \d/i,
    );
    expect(document.body.querySelectorAll("svg")).toHaveLength(0);
  });
});
