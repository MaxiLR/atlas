import { render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { CatalogVisual } from "@/catalog/catalog-visual";
import { CaptionInFrame } from "@/catalog/frame-context";
import { Figure } from "@/components/ui/figure";
import {
  applyStyles,
  clearStyles,
  literalColorsIn,
  sameColor,
  schemes,
  token,
  tokenOf,
} from "@/test/styles";

afterEach(clearStyles);

const CAPTION = "An oceanic plate subducting beneath a continental plate.";

/** A figure the core's sanitiser lets through: shapes, faces and labels. */
const ALLOWED = [
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 680 270">',
  '<path aria-label="Sea level line" d="M0 70H322" stroke-dasharray="2 4"/>',
  '<path aria-label="Slab" class="face-a" d="M0 96H300L560 250H505L290 124H0Z"/>',
  '<path aria-label="Continent" class="face-b" d="M344 101 380 64 476 58H680V150H424Z"/>',
  '<rect aria-label="Mantle" class="face-c" x="0" y="200" width="680" height="70"/>',
  '<circle aria-label="Volcanic arc" cx="502" cy="28" r="6" fill="var(--atlas-accent)"/>',
  '<path aria-label="Melt path" d="M504 205 502 62" stroke="currentColor"/>',
  '<text x="24" y="62">Sea level</text>',
  '<text x="292" y="84">Trench</text>',
  "</svg>",
].join("");

function figure(svg: string, caption = CAPTION): unknown {
  return {
    root: "drawing",
    elements: { drawing: { type: "figure", props: { svg, caption } } },
  };
}

/** A figure as the section view shows it: inside the figure frame. */
function renderFramed(spec: unknown) {
  render(
    <Figure label="Fig. 1" caption={CAPTION}>
      <CaptionInFrame value={true}>
        <CatalogVisual spec={spec} />
      </CaptionInFrame>
    </Figure>,
  );
  return screen.getByRole("figure");
}

/**
 * The colour a shape is painted with. jsdom applies the stylesheets but not
 * SVG presentation attributes, so a shape's own `fill` or `stroke` attribute
 * counts as a browser would count it where no class of the app paints that
 * shape; `currentColor` and the theme's variables resolve where it is.
 */
function paint(element: Element, property: "fill" | "stroke"): string {
  const style = getComputedStyle(element);
  const value = (element.getAttribute(property) ?? style[property]).trim();
  if (value.toLowerCase() === "currentcolor") return style.color;
  const variable = /^var\((--[\w-]+)\)$/.exec(value);
  if (variable) return style.getPropertyValue(variable[1]).trim();
  return value;
}

/** A shape of the figure, by the label the figure gives it. */
function shape(root: Element, name: string): Element {
  const found = root.querySelector(`[aria-label="${name}"]`);
  if (found === null) throw new Error(`no shape "${name}"`);
  return found;
}

const SHAPES = [
  "Sea level line",
  "Melt path",
  "Slab",
  "Continent",
  "Mantle",
  "Volcanic arc",
];
const FACES = ["Slab", "Continent", "Mantle", "Volcanic arc"];

/** Every token-named colour the figure's shapes and labels are painted with. */
function tokenPaints(svg: Element): Record<string, string> {
  const paints: Record<string, string> = {};
  for (const name of SHAPES) {
    const element = shape(svg, name);
    paints[`${name} stroke`] =
      tokenOf(paint(element, "stroke"), element) ?? paint(element, "stroke");
  }
  for (const name of FACES) {
    const element = shape(svg, name);
    paints[`${name} fill`] =
      tokenOf(paint(element, "fill"), element) ?? paint(element, "fill");
  }
  const label = within(svg as HTMLElement).getByText("Sea level");
  paints["label fill"] =
    tokenOf(paint(label, "fill"), label) ?? paint(label, "fill");
  return paints;
}

describe.each(schemes)("a figure in the %s theme", (scheme) => {
  it("draws an allowed figure inline: strokes in the text colour, faces from the surface tokens, labels muted", async () => {
    const frame = renderFramed(figure(ALLOWED));
    await applyStyles(scheme);

    const svg = within(frame).getByRole("img", { name: CAPTION });
    expect(svg.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(within(frame).getByText("Sea level")).toBeInTheDocument();
    expect(within(frame).getByText("Trench")).toBeInTheDocument();

    expect(tokenPaints(svg)).toEqual({
      "Sea level line stroke": "body",
      "Melt path stroke": "body",
      "Slab stroke": "body",
      "Continent stroke": "body",
      "Mantle stroke": "body",
      "Volcanic arc stroke": "body",
      "Slab fill": "raised",
      "Continent fill": "canvas",
      "Mantle fill": "panel",
      "Volcanic arc fill": "accent",
      "label fill": "muted",
    });
    // An unpainted line is drawn, not filled.
    expect(paint(shape(svg, "Sea level line"), "fill")).toBe("none");
    expect(literalColorsIn(frame)).toEqual([]);
  });

  it("sets its labels in the uppercase mono label style and scales to the column", async () => {
    const frame = renderFramed(figure(ALLOWED));
    await applyStyles(scheme);

    const label = getComputedStyle(within(frame).getByText("Trench"));
    expect(label.fontFamily).toMatch(/^"?IBM Plex Mono/);
    expect(label.fontSize).toBe("12px");
    expect(label.textTransform).toBe("uppercase");

    const svg = getComputedStyle(within(frame).getByRole("img"));
    expect(svg.width).toBe("100%");
    expect(svg.height).toBe("auto");
  });

  it("is named by the SVG's title when it has one", async () => {
    const titled = ALLOWED.replace(
      "<path",
      "<title>Cross-section of a subduction zone</title><path",
    );
    const frame = renderFramed(figure(titled));
    await applyStyles(scheme);

    expect(
      within(frame).getByRole("img", {
        name: "Cross-section of a subduction zone",
      }),
    ).toBeInTheDocument();
  });

  it("repaints in the other theme's tokens when the theme changes", async () => {
    const frame = renderFramed(figure(ALLOWED));
    await applyStyles(scheme);
    const svg = within(frame).getByRole("img");
    const rise = shape(svg, "Melt path");
    const slab = shape(svg, "Slab");
    const before = [paint(rise, "stroke"), paint(slab, "fill")];
    expect(sameColor(before[0], token(rise, "body"))).toBe(true);
    expect(sameColor(before[1], token(slab, "raised"))).toBe(true);

    const other = scheme === "light" ? "dark" : "light";
    await applyStyles(other);

    const after = [paint(rise, "stroke"), paint(slab, "fill")];
    expect(sameColor(after[0], token(rise, "body"))).toBe(true);
    expect(sameColor(after[1], token(slab, "raised"))).toBe(true);
    expect(sameColor(after[0], before[0])).toBe(false);
    expect(sameColor(after[1], before[1])).toBe(false);
    expect(tokenPaints(svg)["label fill"]).toBe("muted");
  });
});

/** A figure as if the core's sanitiser had let everything through. */
const HOSTILE = [
  '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 100 40" onload="window.hostile()">',
  "<script>window.hostile()</script>",
  '<svg:script xmlns:svg="http://www.w3.org/2000/svg">window.hostile()</svg:script>',
  "<style>@import url(https://evil.example/theme.css); path { fill: red }</style>",
  '<path aria-label="Kept line" d="M0 20H100" stroke="currentColor" onclick="window.hostile()" onmouseover="window.hostile()"/>',
  '<path d="M0 30H100" fill="url(https://evil.example/paint.svg#p)" style="stroke:url(https://evil.example/s.svg#s);fill:#ff0000"/>',
  '<foreignObject width="100" height="40"><div xmlns="http://www.w3.org/1999/xhtml"><img src="https://evil.example/pixel.png" onerror="window.hostile()"/>Inside a foreign object</div></foreignObject>',
  '<image href="https://evil.example/photo.png" width="10" height="10"/>',
  '<image xlink:href="https://evil.example/photo.png" width="10" height="10"/>',
  '<use href="https://evil.example/sprite.svg#icon"/>',
  '<a href="javascript:window.hostile()"><text x="4" y="16">Click me</text></a>',
  '<animate attributeName="href" to="javascript:window.hostile()"/>',
  '<set attributeName="onclick" to="window.hostile()"/>',
  '<iframe src="https://evil.example/"/>',
  '<text x="4" y="36" style="fill:var(--atlas-muted)">Safe label</text>',
  "</svg>",
].join("");

/** Every attribute in the subtree, as `element@name=value`. */
function attributesIn(root: Element): string[] {
  return [root, ...root.querySelectorAll("*")].flatMap((element) =>
    [...element.attributes].map(
      (attribute) =>
        `${element.localName}@${attribute.name}=${attribute.value}`,
    ),
  );
}

describe("a figure string with active or outside parts", () => {
  it("renders with none of them in the document", async () => {
    const hostile = vi.fn();
    Object.assign(window, { hostile });
    const frame = renderFramed(figure(HOSTILE));
    await applyStyles("dark");

    const svg = within(frame).getByRole("img", { name: CAPTION });
    expect(shape(svg, "Kept line")).toBeInTheDocument();
    expect(within(frame).getByText("Safe label")).toBeInTheDocument();

    for (const element of [
      "script",
      "style",
      "foreignObject",
      "image",
      "use",
      "a",
      "animate",
      "set",
      "iframe",
      "img",
    ]) {
      expect(
        document.body.getElementsByTagName(element),
        `<${element}> in the document`,
      ).toHaveLength(0);
    }
    expect(frame).not.toHaveTextContent("Inside a foreign object");
    expect(
      [...svg.querySelectorAll("*")].filter(
        (element) => element.namespaceURI !== svg.namespaceURI,
      ),
    ).toEqual([]);

    const attributes = attributesIn(svg);
    expect(attributes.filter((entry) => /@on/i.test(entry))).toEqual([]);
    expect(attributes.filter((entry) => /@(xlink:)?href=/.test(entry))).toEqual(
      [],
    );
    expect(attributes.filter((entry) => /url\(\s*(?!#)/i.test(entry))).toEqual(
      [],
    );
    expect(attributes.filter((entry) => /javascript:/i.test(entry))).toEqual(
      [],
    );
    expect(literalColorsIn(frame)).toEqual([]);
    expect(paint(shape(svg, "Kept line"), "fill")).toBe("none");

    for (const element of svg.querySelectorAll("*")) {
      element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      element.dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));
    }
    svg.dispatchEvent(new Event("load"));
    expect(hostile).not.toHaveBeenCalled();
  });

  it("draws nothing from a string that is not SVG, and dumps no parser error", async () => {
    const frame = renderFramed(
      figure('<svg xmlns="http://www.w3.org/2000/svg"><script>window.x()'),
    );
    await applyStyles("dark");

    expect(frame.querySelector("svg")).toBeNull();
    expect(frame).not.toHaveTextContent(/error|parse|line \d/i);
    expect(within(frame).getByText(/could not be drawn/)).toBeVisible();
    expect(within(frame).getByText(CAPTION)).toBeVisible();
  });
});
