/**
 * The unified plugins the prose renderer adds to Markdown: citation markers
 * at their offsets, readable maths errors, marks over quoted text and the
 * block each top-level element belongs to.
 */
import type {
  Element,
  ElementContent,
  Properties,
  Root,
  RootContent,
  Text,
} from "hast";
import type {
  Parent as MdastParent,
  PhrasingContent,
  Root as MdastRoot,
  Text as MdastText,
} from "mdast";

import type { Citation } from "@/core/generated/Citation";

/** A passage of a prose block to show through a supplied component. */
export interface Mark {
  id: string;
  /** What it marks, such as a defined term or a side chat's passage. */
  kind: string;
  block: number;
  /**
   * The text as a selection of it reports it: formulas as their TeX source,
   * citation markers left out.
   */
  quote: string;
}

// -- Citations (Markdown tree) ------------------------------------------------

/** A run of shown text, inline code or an inline formula: what a claim can end on. */
type Leaf = Extract<
  PhrasingContent,
  { type: "text" | "inlineCode" | "inlineMath" }
>;

interface Located {
  node: Leaf;
  parent: MdastParent;
}

function leavesIn(parent: MdastParent, found: Located[] = []): Located[] {
  for (const child of parent.children) {
    if (
      child.type === "text" ||
      child.type === "inlineCode" ||
      child.type === "inlineMath"
    ) {
      found.push({ node: child, parent });
    } else if ("children" in child) {
      leavesIn(child, found);
    }
  }
  return found;
}

function citationMarker(source: number): PhrasingContent {
  return {
    type: "citationMarker",
    children: [],
    data: { hName: "cite", hProperties: { dataSource: source } },
  } as unknown as PhrasingContent;
}

/** ASCII punctuation, which a backslash escapes. */
const ESCAPABLE = /^[!-/:-@[-`{-~]$/;
const REFERENCE =
  /^&(?:#(\d{1,7})|#[xX]([0-9a-fA-F]{1,6})|[a-zA-Z][a-zA-Z0-9]{0,31});/;

/** How many characters of `value` a character reference at `at` decodes to. */
function decodedLength(
  reference: RegExpExecArray,
  value: string,
  at: number,
  next: string | undefined,
): number {
  const [, decimal, hexadecimal] = reference;
  if (decimal !== undefined || hexadecimal !== undefined) {
    const code =
      decimal !== undefined ? Number(decimal) : parseInt(hexadecimal!, 16);
    return code > 0xffff && code <= 0x10ffff ? 2 : 1;
  }
  // A few named references decode to two characters.
  return next !== undefined && value[at + 1] !== next && value[at + 2] === next
    ? 2
    : 1;
}

/**
 * How much of a text node's `value` the first `cut` characters of its
 * `source` show. Escapes and character references count as written, and
 * syntax the value leaves out (a list's indent, a quote's marker) as nothing.
 */
function shownLength(source: string, value: string, cut: number): number {
  let s = 0;
  let v = 0;
  while (s < cut && v < value.length) {
    if (
      source[s] === "\\" &&
      ESCAPABLE.test(source[s + 1] ?? "") &&
      value[v] === source[s + 1]
    ) {
      s += 2;
      v += 1;
      continue;
    }
    const reference =
      source[s] === "&" ? REFERENCE.exec(source.slice(s)) : null;
    if (reference !== null && !value.startsWith(reference[0], v)) {
      s += reference[0].length;
      v += decodedLength(reference, value, v, source[s]);
      continue;
    }
    if (source[s] === value[v]) v += 1;
    s += 1;
  }
  return v;
}

/**
 * Where in `text`, in UTF-16 units, its first `characters` Unicode
 * characters end: citations count characters, as the core does.
 */
export function utf16Offset(text: string, characters: number): number {
  let offset = 0;
  for (let read = 0; read < characters && offset < text.length; read += 1) {
    offset += text.codePointAt(offset)! > 0xffff ? 2 : 1;
  }
  return offset;
}

/**
 * Puts a marker for each citation where its claim ends. Offsets count
 * Unicode characters of the block's source text, as the core places them; a
 * claim that ends on inline code or a formula has its marker after it.
 */
export function remarkCitations({ citations }: { citations: Citation[] }) {
  return (tree: MdastRoot, file: { value: unknown }) => {
    const source = String(file.value);
    const latestFirst = [...citations].sort((a, b) => b.end - a.end);
    for (const citation of latestFirst) {
      const end = utf16Offset(source, citation.end);
      let target: Located | undefined;
      for (const located of leavesIn(tree)) {
        const start = located.node.position?.start.offset;
        if (start === undefined) continue;
        if (start >= end) break;
        target = located;
      }
      if (target === undefined) continue;
      const { node, parent } = target;
      const children = parent.children as PhrasingContent[];
      const at = children.indexOf(node);
      const marker = citationMarker(citation.source);
      const start = node.position!.start.offset!;
      const cut =
        node.type === "text"
          ? shownLength(
              source.slice(start, node.position!.end.offset),
              node.value,
              end - start,
            )
          : node.value.length;
      if (cut >= node.value.length) {
        children.splice(at + 1, 0, marker);
      } else {
        const rest: MdastText = { type: "text", value: node.value.slice(cut) };
        node.value = node.value.slice(0, cut);
        children.splice(at + 1, 0, marker, rest);
      }
    }
  };
}

// -- Formulas (HTML tree) -----------------------------------------------------

function classesOf(element: Element): string[] {
  const names = element.properties.className;
  return Array.isArray(names) ? names.map(String) : [];
}

function textOf(node: ElementContent | Root): string {
  if (node.type === "text") return node.value;
  if ("children" in node) {
    return node.children
      .map((child) => textOf(child as ElementContent))
      .join("");
  }
  return "";
}

/** KaTeX's reason without its prefix and the source it quotes. */
function reasonOf(title: string): string {
  return title
    .replace(/^[\s\S]*?parse error: /i, "")
    .replace(/ at (?:position \d+|end of input):[\s\S]*$/, "")
    .replace(/^\w*Error: /, "");
}

function text(value: string): Text {
  return { type: "text", value };
}

function element(
  tagName: string,
  properties: Properties,
  children: ElementContent[],
): Element {
  return { type: "element", tagName, properties, children };
}

function find(node: Element, tagName: string): Element | undefined {
  for (const child of node.children) {
    if (child.type !== "element") continue;
    if (child.tagName === tagName) return child;
    const found = find(child, tagName);
    if (found !== undefined) return found;
  }
  return undefined;
}

/**
 * Names each formula's TeX source in `data-formula`, which is how a
 * selection and a mark read it, and lets a display formula wider than the
 * column scroll sideways inside it. A formula KaTeX cannot render becomes a
 * sentence saying so and its source; while the block streams, a formula
 * still arriving shows as its source alone.
 */
export function rehypeFormulas({ streaming }: { streaming: boolean }) {
  const visit = (parent: Root | Element) => {
    parent.children.forEach((child, at) => {
      if (child.type !== "element") return;
      const classes = classesOf(child);
      if (classes.includes("katex-display") || classes.includes("katex")) {
        const annotation = find(child, "annotation");
        child.properties.dataFormula =
          annotation === undefined ? "" : textOf(annotation);
        if (classes.includes("katex-display")) {
          child.properties.className = [
            ...classes,
            "overflow-x-auto",
            "overflow-y-hidden",
            "py-1",
          ];
        }
        return;
      }
      if (!classes.includes("katex-error")) {
        visit(child);
        return;
      }
      const formula = textOf(child);
      const source = element("code", {}, [text(formula)]);
      const display = parent.type === "root";
      const shown = streaming
        ? source
        : element(
            display ? "div" : "span",
            {
              dataFormula: formula,
              className: display
                ? ["mt-(--typeset-flow)", "text-15", "text-muted-foreground"]
                : ["text-muted-foreground"],
            },
            [
              text(
                `This formula could not be shown: ${reasonOf(String(child.properties.title ?? ""))}. `,
              ),
              source,
            ],
          );
      parent.children[at] = shown;
    });
  };
  return (tree: Root) => visit(tree);
}

// -- Marks (HTML tree) --------------------------------------------------------

/** Containers whose whitespace between children is layout, not text. */
const LAYOUT: Record<string, true> = {
  ul: true,
  ol: true,
  table: true,
  thead: true,
  tbody: true,
  tfoot: true,
  tr: true,
  blockquote: true,
};

/**
 * A piece of a block's text as a selection reads it: a text node, or a
 * formula, which reads as its TeX source.
 */
interface Piece {
  node: Text | Element;
  parent: Root | Element;
  value: string;
  /** Where it starts in the block's text. */
  from: number;
}

function piecesIn(
  parent: Root | Element,
  pieces: Piece[],
  length: { at: number },
) {
  for (const child of parent.children) {
    if (child.type === "text") {
      pieces.push({ node: child, parent, value: child.value, from: length.at });
      length.at += child.value.length;
    } else if (child.type === "element" && child.tagName !== "cite") {
      const formula = child.properties.dataFormula;
      if (formula === undefined) {
        piecesIn(child, pieces, length);
      } else {
        const value = String(formula);
        pieces.push({ node: child, parent, value, from: length.at });
        length.at += value.length;
      }
    }
  }
  return pieces;
}

/** The text with whitespace runs as one space, and where each character came from. */
function collapsed(value: string): { text: string; origin: number[] } {
  let text = "";
  const origin: number[] = [];
  for (let at = 0; at < value.length; at += 1) {
    if (/\s/.test(value[at]!)) {
      if (text.endsWith(" ")) continue;
      text += " ";
    } else {
      text += value[at];
    }
    origin.push(at);
  }
  return { text, origin };
}

/** The parts a piece becomes when the mark covers `from` to `to` of it. */
function marked(piece: Piece, from: number, to: number, mark: Mark) {
  const properties = { dataMarkId: mark.id, dataMarkKind: mark.kind };
  if (piece.node.type === "element") {
    // An inline formula is marked whole; a display formula is a block of
    // its own and stays unmarked.
    const inline =
      piece.node.tagName === "span" &&
      !classesOf(piece.node).includes("katex-display");
    return inline ? [element("mark", properties, [piece.node])] : null;
  }
  const { value } = piece;
  const inside = value.slice(from, to);
  const layout =
    piece.parent.type === "root" || LAYOUT[piece.parent.tagName] === true;
  if (layout && inside.trim() === "") return null;
  const parts: ElementContent[] = [];
  if (from > 0) parts.push(text(value.slice(0, from)));
  parts.push(element("mark", properties, [text(inside)]));
  if (to < value.length) parts.push(text(value.slice(to)));
  return parts;
}

function wrap(tree: Root, mark: Mark) {
  const pieces = piecesIn(tree, [], { at: 0 });
  const all = pieces.map((piece) => piece.value).join("");
  const quote = mark.quote.replace(/\s+/g, " ").trim();
  if (quote === "") return;
  const { text: flat, origin } = collapsed(all);
  const found = flat.indexOf(quote);
  if (found === -1) return;
  const start = origin[found]!;
  const end = origin[found + quote.length - 1]! + 1;

  for (const piece of [...pieces].reverse()) {
    const from = Math.max(start, piece.from) - piece.from;
    const to = Math.min(end, piece.from + piece.value.length) - piece.from;
    if (from >= to) continue;
    const parts = marked(piece, from, to, mark);
    if (parts === null) continue;
    const siblings = piece.parent.children as (RootContent | ElementContent)[];
    siblings.splice(siblings.indexOf(piece.node), 1, ...parts);
  }
}

/**
 * Wraps each mark's first occurrence in the block, across inline elements
 * and inline formulas, leaving citation markers out.
 */
export function rehypeMarks({ marks }: { marks: Mark[] }) {
  return (tree: Root) => {
    for (const mark of marks) wrap(tree, mark);
  };
}

// -- Blocks (HTML tree) -------------------------------------------------------

/** Names the prose block each top-level element belongs to. */
export function rehypeProseBlock({ block }: { block: number }) {
  return (tree: Root) => {
    for (const child of tree.children) {
      if (child.type === "element") child.properties.dataProseBlock = block;
    }
  };
}
