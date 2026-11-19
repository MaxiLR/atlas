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
  /** The text as shown, as a selection of it reads. */
  quote: string;
}

// -- Citations (Markdown tree) ------------------------------------------------

interface Located {
  node: MdastText;
  parent: MdastParent;
}

function textsIn(parent: MdastParent, found: Located[] = []): Located[] {
  for (const child of parent.children) {
    if (child.type === "text") found.push({ node: child, parent });
    else if ("children" in child) textsIn(child, found);
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

/**
 * Puts a marker for each citation where its claim ends. Offsets count
 * characters of the block's source text, as the core places them.
 */
export function remarkCitations({ citations }: { citations: Citation[] }) {
  return (tree: MdastRoot) => {
    const latestFirst = [...citations].sort((a, b) => b.end - a.end);
    for (const citation of latestFirst) {
      let target: Located | undefined;
      for (const located of textsIn(tree)) {
        const start = located.node.position?.start.offset;
        if (start === undefined) continue;
        if (start >= citation.end) break;
        target = located;
      }
      if (target === undefined) continue;
      const { node, parent } = target;
      const children = parent.children as PhrasingContent[];
      const at = children.indexOf(node);
      const cut = citation.end - node.position!.start.offset!;
      const marker = citationMarker(citation.source);
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

// -- Maths errors (HTML tree) -------------------------------------------------

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

/**
 * Replaces the coloured source KaTeX leaves for a formula it cannot render
 * with a sentence saying so and the source. While the block streams, a
 * formula still arriving shows as its source alone.
 */
export function rehypeMathErrors({ streaming }: { streaming: boolean }) {
  const replace = (parent: Root | Element) => {
    parent.children.forEach((child, at) => {
      if (child.type !== "element") return;
      if (!classesOf(child).includes("katex-error")) {
        replace(child);
        return;
      }
      const source = element("code", {}, [text(textOf(child))]);
      const display = parent.type === "root";
      const shown = streaming
        ? source
        : element(
            display ? "div" : "span",
            {
              dataMathError: "",
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
  return (tree: Root) => replace(tree);
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

interface Run {
  node: Text;
  parent: Root | Element;
  /** Where its text starts in the block's text. */
  from: number;
}

/** Elements whose text is not prose: formulas and citation markers. */
function opaque(node: Element): boolean {
  if (node.tagName === "cite" || "dataMathError" in node.properties) {
    return true;
  }
  return classesOf(node).some((name) => name.startsWith("katex"));
}

function runsIn(parent: Root | Element, runs: Run[], length: { at: number }) {
  for (const child of parent.children) {
    if (child.type === "text") {
      runs.push({ node: child, parent, from: length.at });
      length.at += child.value.length;
    } else if (child.type === "element" && !opaque(child)) {
      runsIn(child, runs, length);
    }
  }
  return runs;
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

function wrap(tree: Root, mark: Mark) {
  const runs = runsIn(tree, [], { at: 0 });
  const all = runs.map((run) => run.node.value).join("");
  const quote = mark.quote.replace(/\s+/g, " ").trim();
  if (quote === "") return;
  const { text: flat, origin } = collapsed(all);
  const found = flat.indexOf(quote);
  if (found === -1) return;
  const start = origin[found]!;
  const end = origin[found + quote.length - 1]! + 1;

  for (const run of [...runs].reverse()) {
    const from = Math.max(start, run.from) - run.from;
    const to = Math.min(end, run.from + run.node.value.length) - run.from;
    if (from >= to) continue;
    const value = run.node.value;
    const inside = value.slice(from, to);
    const layout =
      run.parent.type === "root" || LAYOUT[run.parent.tagName] === true;
    if (layout && inside.trim() === "") continue;
    const parts: ElementContent[] = [];
    if (from > 0) parts.push(text(value.slice(0, from)));
    parts.push(
      element("mark", { dataMarkId: mark.id, dataMarkKind: mark.kind }, [
        text(inside),
      ]),
    );
    if (to < value.length) parts.push(text(value.slice(to)));
    const siblings = run.parent.children as (RootContent | ElementContent)[];
    siblings.splice(siblings.indexOf(run.node), 1, ...parts);
  }
}

/** Wraps each mark's first occurrence in the block, across inline elements. */
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
