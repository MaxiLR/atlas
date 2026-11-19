/**
 * The interface's second line of defence for figures. The core sends only
 * SVG its sanitiser passed (`crates/atlas-core/src/sanitise/`); this parses
 * it again in an inert document and builds a fresh SVG element from what the
 * same allow-list keeps, so markup the sanitiser should have removed never
 * reaches the page. Every element is created anew: nothing of the parsed
 * document is inserted.
 */

const SVG = "http://www.w3.org/2000/svg";

/** The colour tokens of the theme generator, as their CSS names. */
const TOKENS = new Set([
  "canvas",
  "panel",
  "raised",
  "heading",
  "body",
  "muted",
  "faint",
  "edge",
  "edge-strong",
  "wash",
  "wash-strong",
  "accent",
  "accent-wash",
  "primary-fill",
  "primary-text",
  "focus",
  "green",
  "amber",
  "red",
]);

/** The classes a figure may use for its faces; the figure styles them. */
const FACES = new Set(["face-a", "face-b", "face-c"]);

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
  "clipPath",
  "linearGradient",
  "stop",
]);

/** Elements whose text is kept. */
const TEXT_ELEMENTS = new Set(["text", "tspan", "title", "desc"]);

/** Definitions a `url(#id)` may name, and so the only elements with ids. */
const DEFINITIONS = new Set(["linearGradient", "clipPath", "marker"]);

/** The deepest nesting kept, the root counting as one. */
const MAX_DEPTH = 32;

function allowedIn(name: string, parent: string | null): boolean {
  if (parent === null) return name === "svg";
  if (name === "svg") return false;
  if (name === "stop") return parent === "linearGradient";
  if (parent === "linearGradient") return false;
  if (name === "tspan") return parent === "text" || parent === "tspan";
  return !TEXT_ELEMENTS.has(parent);
}

type Kind =
  | "lengths"
  | "numbers"
  | "path"
  | "transform"
  | "dasharray"
  | "orient"
  | "paint"
  | "stop-color"
  | "class"
  | "label"
  | "id"
  | "style"
  | { keyword: readonly string[] }
  | { reference: string };

const ROLES = ["img", "presentation", "none", "group"];
const RULES = ["nonzero", "evenodd"];
const UNITS = ["userSpaceOnUse", "objectBoundingBox"];

/** The presentation properties, as attributes and inside `style`. */
function property(name: string): Kind | null {
  switch (name) {
    case "fill":
    case "stroke":
      return "paint";
    case "fill-opacity":
    case "stroke-opacity":
    case "opacity":
    case "stroke-width":
    case "stroke-miterlimit":
    case "stroke-dashoffset":
    case "font-size":
      return "lengths";
    case "fill-rule":
    case "clip-rule":
      return { keyword: RULES };
    case "stroke-linecap":
      return { keyword: ["butt", "round", "square"] };
    case "stroke-linejoin":
      return { keyword: ["miter", "round", "bevel", "miter-clip", "arcs"] };
    case "stroke-dasharray":
      return "dasharray";
    case "text-anchor":
      return { keyword: ["start", "middle", "end"] };
    case "clip-path":
      return { reference: "clipPath" };
    case "marker-start":
    case "marker-mid":
    case "marker-end":
      return { reference: "marker" };
    default:
      return null;
  }
}

/** What `attribute` must hold on `element`; null when it is not allowed. */
function attributeKind(element: string, attribute: string): Kind | null {
  const specific: Partial<Record<string, Partial<Record<string, Kind>>>> = {
    svg: { viewBox: "numbers", width: "lengths", height: "lengths" },
    marker: {
      viewBox: "numbers",
      refX: "lengths",
      refY: "lengths",
      markerWidth: "lengths",
      markerHeight: "lengths",
      markerUnits: { keyword: ["strokeWidth", "userSpaceOnUse"] },
      orient: "orient",
      id: "id",
    },
    path: { d: "path" },
    rect: {
      x: "lengths",
      y: "lengths",
      width: "lengths",
      height: "lengths",
      rx: "lengths",
      ry: "lengths",
    },
    circle: { cx: "lengths", cy: "lengths", r: "lengths" },
    ellipse: { cx: "lengths", cy: "lengths", rx: "lengths", ry: "lengths" },
    line: { x1: "lengths", y1: "lengths", x2: "lengths", y2: "lengths" },
    linearGradient: {
      x1: "lengths",
      y1: "lengths",
      x2: "lengths",
      y2: "lengths",
      gradientUnits: { keyword: UNITS },
      gradientTransform: "transform",
      id: "id",
    },
    polyline: { points: "numbers" },
    polygon: { points: "numbers" },
    text: { x: "lengths", y: "lengths", dx: "lengths", dy: "lengths" },
    tspan: { x: "lengths", y: "lengths", dx: "lengths", dy: "lengths" },
    clipPath: { clipPathUnits: { keyword: UNITS }, id: "id" },
    stop: {
      offset: "lengths",
      "stop-color": "stop-color",
      "stop-opacity": "lengths",
    },
  };
  const kind = specific[element]?.[attribute];
  if (kind !== undefined) return kind;
  if (["title", "desc", "stop", "linearGradient"].includes(element)) {
    return null;
  }
  switch (attribute) {
    case "transform":
      return element === "svg" ? null : "transform";
    case "role":
      return { keyword: ROLES };
    case "aria-label":
      return "label";
    case "class":
      return "class";
    case "style":
      return "style";
    default:
      return property(attribute);
  }
}

const NUMBER = String.raw`[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?`;
const isNumber = (value: string) => new RegExp(`^${NUMBER}$`).test(value);
const isLength = (value: string) =>
  new RegExp(`^${NUMBER}(?:px|em|%)?$`).test(value);
const items = (value: string) =>
  value.split(/[\s,]+/).filter((item) => item !== "");
const isLengths = (value: string) =>
  value !== "" && items(value).every(isLength);
const isNumbers = (value: string) =>
  value !== "" && items(value).every(isNumber);
const isPathData = (value: string) =>
  value !== "" && /^[MmLlHhVvCcSsQqTtAaZz0-9.,+\-eE\s]+$/.test(value);
const isId = (value: string) => /^[A-Za-z_][\w-]{0,63}$/.test(value);

function isTransform(value: string): boolean {
  const call = String.raw`\s*(?:matrix|translate|scale|rotate|skewX|skewY)\s*\(([^()]*)\)[\s,]*`;
  if (!new RegExp(`^(?:${call})+$`).test(value)) return false;
  return [...value.matchAll(new RegExp(call, "g"))].every((match) =>
    isNumbers(match[1].trim()),
  );
}

function isToken(value: string): boolean {
  const match = /^var\(--atlas-([a-z-]+)\)$/.exec(value);
  return match !== null && TOKENS.has(match[1]);
}

/** The ids of the kept definitions, each with its element name. */
type Definitions = Map<string, string>;

/** Ids are prefixed so two figures on one page never name each other's. */
interface Scope {
  definitions: Definitions;
  prefix: string;
}

function reference(value: string, element: string, scope: Scope) {
  const match = /^url\(#([^)]*)\)$/.exec(value);
  if (match === null || scope.definitions.get(match[1]) !== element) {
    return null;
  }
  return `url(#${scope.prefix}${match[1]})`;
}

/** The value to write for an attribute of `kind`; null drops it. */
function checked(kind: Kind, raw: string, scope: Scope): string | null {
  const value = raw.trim();
  if (typeof kind === "object") {
    if ("keyword" in kind) return kind.keyword.includes(value) ? value : null;
    return value === "none" ? value : reference(value, kind.reference, scope);
  }
  switch (kind) {
    case "lengths":
      return isLengths(value) ? value : null;
    case "numbers":
      return isNumbers(value) ? value : null;
    case "path":
      return isPathData(value) ? value : null;
    case "transform":
      return isTransform(value) ? value : null;
    case "dasharray":
      return value === "none" || isLengths(value) ? value : null;
    case "orient":
      return value === "auto" ||
        value === "auto-start-reverse" ||
        isNumber(value.replace(/deg$/, ""))
        ? value
        : null;
    case "paint":
      if (value === "none" || value === "currentColor" || isToken(value)) {
        return value;
      }
      return reference(value, "linearGradient", scope);
    case "stop-color":
      return value === "currentColor" || isToken(value) ? value : null;
    case "class": {
      const faces = value.split(/\s+/).filter((name) => FACES.has(name));
      return faces.length > 0 ? faces.join(" ") : null;
    }
    case "label":
      return raw;
    case "id":
      return isId(value) ? `${scope.prefix}${value}` : null;
    case "style":
      return style(value, scope);
  }
}

/** Allowed declarations of a `style` attribute; anything beyond plain ones drops it. */
function style(css: string, scope: Scope): string | null {
  if (/[\\@{}<>"']|\/\*/.test(css)) return null;
  const declarations: string[] = [];
  for (const declaration of css.split(";")) {
    if (declaration.trim() === "") continue;
    const colon = declaration.indexOf(":");
    if (colon === -1) return null;
    const name = declaration.slice(0, colon).trim().toLowerCase();
    const kind = property(name);
    if (kind === null) continue;
    const value = checked(kind, declaration.slice(colon + 1), scope);
    if (value !== null) declarations.push(`${name}:${value}`);
  }
  return declarations.length > 0 ? declarations.join(";") : null;
}

/** The allowed SVG children of `element` with their depth checked. */
function keptChildren(element: Element, depth: number): Element[] {
  if (depth >= MAX_DEPTH) return [];
  return [...element.children].filter(
    (child) =>
      child.namespaceURI === SVG &&
      child.prefix === null &&
      ELEMENTS.has(child.localName) &&
      allowedIn(child.localName, element.localName),
  );
}

function collectDefinitions(
  element: Element,
  depth: number,
  into: Definitions,
) {
  const id = element.getAttribute("id")?.trim();
  if (DEFINITIONS.has(element.localName) && id && isId(id) && !into.has(id)) {
    into.set(id, element.localName);
  }
  for (const child of keptChildren(element, depth)) {
    collectDefinitions(child, depth + 1, into);
  }
}

function rebuild(source: Element, depth: number, scope: Scope): Element {
  const name = source.localName;
  const element = document.createElementNS(SVG, name);
  for (const attribute of source.attributes) {
    if (attribute.namespaceURI !== null) continue;
    const kind = attributeKind(name, attribute.name);
    if (kind === null) continue;
    const value = checked(kind, attribute.value, scope);
    if (value !== null) element.setAttribute(attribute.name, value);
  }
  const kept = new Set<Node>(keptChildren(source, depth));
  for (const child of source.childNodes) {
    if (child.nodeType === Node.TEXT_NODE && TEXT_ELEMENTS.has(name)) {
      element.append(document.createTextNode(child.textContent ?? ""));
    } else if (kept.has(child)) {
      element.append(rebuild(child as Element, depth + 1, scope));
    }
  }
  return element;
}

/**
 * The figure `markup` draws, rebuilt from the allow-list, named by its title
 * or else by `caption`; null when it is not an SVG document.
 */
export function inertSvg(
  markup: string,
  caption: string,
  idPrefix: string,
): SVGSVGElement | null {
  const parsed = new DOMParser().parseFromString(markup, "image/svg+xml");
  const root = parsed.documentElement;
  if (
    root.namespaceURI !== SVG ||
    root.localName !== "svg" ||
    root.prefix !== null ||
    parsed.getElementsByTagName("parsererror").length > 0
  ) {
    return null;
  }
  const scope: Scope = { definitions: new Map(), prefix: `${idPrefix}-` };
  collectDefinitions(root, 1, scope.definitions);
  const svg = rebuild(root, 1, scope) as SVGSVGElement;

  const title = [...svg.children]
    .find((child) => child.localName === "title")
    ?.textContent?.trim();
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", title || caption);
  return svg;
}
