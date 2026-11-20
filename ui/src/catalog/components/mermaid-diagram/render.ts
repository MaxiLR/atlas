/**
 * Mermaid at its strict level, themed from the tokens where the diagram
 * sits, with its output made inert before it reaches the page.
 */
import { converter, formatHex } from "culori";
import type { Mermaid, MermaidConfig } from "mermaid";

const SVG = "http://www.w3.org/2000/svg";

const FONT = '"IBM Plex Mono", ui-monospace, Menlo, monospace';

/**
 * Configuration a diagram's own directives or front matter may not change:
 * Mermaid's own secure keys and everything that decides how it draws.
 */
const SECURE = [
  "secure",
  "securityLevel",
  "startOnLoad",
  "maxTextSize",
  "suppressErrorRendering",
  "maxEdges",
  "htmlLabels",
  "theme",
  "themeVariables",
  "themeCSS",
  "darkMode",
  "fontFamily",
  "altFontFamily",
  "fontSize",
  "look",
  "handDrawnSeed",
];

const TOKENS = ["canvas", "panel", "raised", "body", "muted", "faint"] as const;
type Palette = Record<(typeof TOKENS)[number], string>;

const toRgb = converter("rgb");

/**
 * The tokens where `element` sits, as hex for Mermaid's colour maths; null
 * when the theme's variables are not there.
 */
export function paletteAt(element: Element): Palette | null {
  const style = getComputedStyle(element);
  const palette: Partial<Palette> = {};
  for (const name of TOKENS) {
    const rgb = toRgb(style.getPropertyValue(`--atlas-${name}`).trim());
    if (rgb === undefined) return null;
    palette[name] = formatHex(rgb);
  }
  return palette as Palette;
}

/** The figure's look in Mermaid's terms: strokes in the text colour, faces from the surfaces, muted mono labels. */
function themeVariables(palette: Palette, dark: boolean) {
  const { canvas, panel, raised, body, muted, faint } = palette;
  return {
    darkMode: dark,
    fontFamily: FONT,
    fontSize: "12px",
    background: panel,
    primaryColor: raised,
    secondaryColor: canvas,
    tertiaryColor: panel,
    mainBkg: raised,
    secondBkg: canvas,
    primaryBorderColor: body,
    secondaryBorderColor: body,
    tertiaryBorderColor: body,
    nodeBorder: body,
    lineColor: body,
    defaultLinkColor: body,
    arrowheadColor: body,
    primaryTextColor: muted,
    secondaryTextColor: muted,
    tertiaryTextColor: muted,
    textColor: muted,
    nodeTextColor: muted,
    titleColor: muted,
    edgeLabelBackground: panel,
    clusterBkg: canvas,
    clusterBorder: faint,
    noteBkgColor: canvas,
    noteBorderColor: faint,
    noteTextColor: muted,
    actorBkg: raised,
    actorBorder: body,
    actorTextColor: muted,
    actorLineColor: faint,
    signalColor: body,
    signalTextColor: muted,
    labelBoxBkgColor: raised,
    labelBoxBorderColor: body,
    labelTextColor: muted,
    loopTextColor: muted,
    activationBkgColor: canvas,
    activationBorderColor: body,
    sequenceNumberColor: canvas,
    labelColor: muted,
    altBackground: canvas,
    stateBkg: raised,
    stateBorder: body,
    transitionColor: body,
    transitionLabelColor: muted,
    specialStateColor: body,
    innerEndBackground: body,
  };
}

function config(palette: Palette | null, dark: boolean): MermaidConfig {
  return {
    startOnLoad: false,
    securityLevel: "strict",
    htmlLabels: false,
    suppressErrorRendering: true,
    secure: SECURE,
    theme: "base",
    // The plain look: thin strokes and no drop shadows, like the figure.
    look: "classic",
    darkMode: dark,
    fontFamily: FONT,
    fontSize: 12,
    ...(palette === null
      ? {}
      : { themeVariables: themeVariables(palette, dark) }),
  };
}

/** The elements a drawn diagram may hold; `a` keeps its content only. */
const ELEMENTS = new Set([
  "svg",
  "g",
  "path",
  "rect",
  "circle",
  "ellipse",
  "line",
  "polyline",
  "polygon",
  "text",
  "tspan",
  "title",
  "desc",
  "defs",
  "marker",
  "style",
  "clipPath",
  "linearGradient",
  "radialGradient",
  "stop",
  "pattern",
  "filter",
  "feDropShadow",
  "feGaussianBlur",
  "feOffset",
  "feFlood",
  "feComposite",
  "feMerge",
  "feMergeNode",
  "feBlend",
  "feColorMatrix",
]);

/** A `url()` naming anything but a fragment of the same document. */
const OUTSIDE = /url\(\s*['"]?\s*(?!#)[^)]*\)/gi;

function active(name: string, value: string): boolean {
  const lower = name.toLowerCase();
  return (
    lower.startsWith("on") ||
    lower === "href" ||
    lower.endsWith(":href") ||
    lower === "src" ||
    /javascript:|data:/i.test(value) ||
    new RegExp(OUTSIDE.source, "i").test(value)
  );
}

/** Removes what could act, link, fetch or carry HTML. */
function makeInert(element: Element) {
  for (const child of [...element.children]) {
    if (child.namespaceURI !== SVG) {
      child.remove();
    } else if (child.localName === "a") {
      makeInert(child);
      child.replaceWith(...child.childNodes);
    } else if (!ELEMENTS.has(child.localName)) {
      child.remove();
    } else {
      makeInert(child);
    }
  }
  for (const { name, value } of [...element.attributes]) {
    if (active(name, value)) element.removeAttribute(name);
  }
  if (element.localName === "style") {
    element.textContent = (element.textContent ?? "")
      .replace(/@import[^;]*;?/gi, "")
      .replace(OUTSIDE, "none");
  }
}

let mermaid: Promise<Mermaid> | undefined;
let queue: Promise<unknown> = Promise.resolve();
let count = 0;

/** A drawn diagram and the styles Mermaid scoped to its id. */
export interface Diagram {
  svg: SVGSVGElement;
  css: string;
}

/**
 * Draws `source` with the palette, inert and named by its title or else by
 * `caption`; null when Mermaid cannot parse or draw it.
 */
export function renderDiagram(
  source: string,
  caption: string,
  palette: Palette | null,
  dark: boolean,
): Promise<Diagram | null> {
  // Mermaid is large and only a section with a diagram needs it, so it is
  // loaded, from the app's own files, on first use.
  mermaid ??= import("mermaid").then((module) => module.default);
  // Mermaid's configuration is global: one diagram at a time.
  const job = queue.then(async () => {
    const api = await mermaid!;
    api.initialize(config(palette, dark));
    if ((await api.parse(source, { suppressErrors: true })) === false) {
      return null;
    }
    count += 1;
    const { svg } = await api.render(`atlas-mermaid-${count}`, source);
    const parsed = new DOMParser().parseFromString(svg, "text/html");
    const root = parsed.body.firstElementChild;
    if (
      root === null ||
      root.namespaceURI !== SVG ||
      root.localName !== "svg"
    ) {
      return null;
    }
    makeInert(root);
    // The diagram's styles, scoped to its id, go beside it in the page.
    const styles = [...root.querySelectorAll("style")];
    const css = styles.map((style) => style.textContent ?? "").join("\n");
    for (const style of styles) style.remove();
    const title = [...root.children]
      .find((child) => child.localName === "title")
      ?.textContent?.trim();
    for (const name of [
      "aria-labelledby",
      "aria-describedby",
      "aria-roledescription",
    ]) {
      root.removeAttribute(name);
    }
    root.setAttribute("role", "img");
    root.setAttribute("aria-label", title || caption);
    return {
      svg: document.importNode(root, true) as SVGSVGElement,
      css,
    };
  });
  queue = job.catch(() => undefined);
  return job.catch(() => null);
}
