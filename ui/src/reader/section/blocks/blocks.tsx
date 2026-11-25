import {
  Fragment,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  type ComponentType,
  type ReactNode,
} from "react";

import { CatalogVisual } from "@/catalog/catalog-visual";
import { CaptionInFrame, FrameDescription } from "@/catalog/frame-context";
import { checkVisualSpec } from "@/catalog/visual-spec";
import { Figure } from "@/components/ui/figure";
import { Skeleton } from "@/components/ui/skeleton";
import type { BlockContent } from "@/core/generated/BlockContent";
import type { BlockKind } from "@/core/generated/BlockKind";
import type { Citation } from "@/core/generated/Citation";
import type { Source } from "@/core/generated/Source";
import type { ShownBlock } from "@/store";
import { unknownTag } from "@/store/unknown-tag";

import { Check, type CheckBlock } from "./check-block";
import type { Mark } from "./markdown-plugins";
import { Prose, type MarkProps } from "./prose";
import { RegistryEntry } from "./registry-entry";

export type { Mark } from "./markdown-plugins";
export type { MarkProps } from "./prose";

/** What a selection in the block list covers. */
export interface Passage {
  /** The prose block it lies in. */
  block: number;
  /**
   * The selected text as it reads, formulas as their TeX source and
   * citation markers left out: given back as a mark's quote, it marks the
   * same passage.
   */
  text: string;
}

/** The series a framed block is numbered in, as the draft labels them. */
type Series = "Fig." | "Table";

/**
 * How a complete block sits in the column: prose, a framed visual with its
 * series, or an unframed catalog entry (a callout, a check question).
 */
function seriesOf(content: BlockContent): Series | null {
  switch (content.type) {
    case "Prose":
    case "Check":
      return null;
    case "Visual": {
      const check = checkVisualSpec(content.spec);
      if (!check.ok) return "Fig.";
      const type = check.spec.elements[check.spec.root]?.type;
      if (type === "callout") return null;
      return type === "comparison" ? "Table" : "Fig.";
    }
    case "Figure":
    case "Mermaid":
    case "Fallback":
      return "Fig.";
    default:
      return unknownTag(content);
  }
}

/** The label of each framed block, numbered in its own series. */
function labelsOf(blocks: ShownBlock[]): Map<number, string> {
  const counts: Record<Series, number> = { "Fig.": 0, Table: 0 };
  const labels = new Map<number, string>();
  for (const block of blocks) {
    if (block.state !== "complete") continue;
    const series = seriesOf(block.content);
    if (series === null) continue;
    counts[series] += 1;
    labels.set(block.index, `${series} ${counts[series]}`);
  }
  return labels;
}

const placeholderText: Record<Exclude<BlockKind, "Prose">, string> = {
  Visual: "Drawing the visual…",
  Figure: "Drawing the figure…",
  Mermaid: "Drawing the diagram…",
  Check: "Writing the check question…",
};

/**
 * Holds an opened visual's place at a fixed height: what streams into it is
 * never shown, so it does not grow, and the block replaces it in place.
 */
function Placeholder({ kind }: { kind: Exclude<BlockKind, "Prose"> }) {
  return (
    <div
      data-slot="block-placeholder"
      className="flex h-60 flex-col overflow-hidden rounded-xl border border-edge bg-panel"
    >
      <div className="flex-1 p-4">
        <Skeleton className="size-full rounded-lg" />
      </div>
      <p className="border-t border-edge px-4 py-2.5 text-13 text-muted-foreground">
        {placeholderText[kind]}
      </p>
    </div>
  );
}

/**
 * The shared figure frame around a catalog entry: the frame shows the
 * number and the caption, and the entry inside shows its content only,
 * with the block's description at hand for when it cannot be drawn.
 */
function Framed({
  label,
  caption,
  description,
  children,
}: {
  label: string;
  caption: string;
  description: string | null;
  children: ReactNode;
}) {
  return (
    <Figure label={label} caption={caption}>
      <CaptionInFrame value={true}>
        <FrameDescription value={description}>{children}</FrameDescription>
      </CaptionInFrame>
    </Figure>
  );
}

/** A visual that could not be drawn, told by its description. */
function Described({
  label,
  description,
}: {
  label: string;
  description: string;
}) {
  return (
    <Figure label={label} caption={description}>
      <p className="px-4 py-5 text-13 text-muted-foreground">
        This visual could not be drawn; its description is below.
      </p>
    </Figure>
  );
}

function captionOf(spec: unknown, description: string | null): string {
  const check = checkVisualSpec(spec);
  if (check.ok) {
    const caption = check.spec.elements[check.spec.root]?.props.caption;
    if (typeof caption === "string") return caption;
  }
  return description ?? "";
}

function Complete({
  content,
  label,
  prose,
  at,
}: {
  content: BlockContent;
  label: string | undefined;
  prose: (text: string) => ReactNode;
  at: CheckBlock;
}) {
  switch (content.type) {
    case "Prose":
      return prose(content.text);
    case "Visual": {
      if (!checkVisualSpec(content.spec).ok) {
        return (
          <Described
            label={label ?? ""}
            description={content.description ?? ""}
          />
        );
      }
      if (label === undefined) return <CatalogVisual spec={content.spec} />;
      return (
        <Framed
          label={label}
          caption={captionOf(content.spec, content.description)}
          description={content.description}
        >
          <CatalogVisual spec={content.spec} />
        </Framed>
      );
    }
    case "Figure":
      return (
        <Framed
          label={label ?? ""}
          caption={content.description ?? ""}
          description={content.description}
        >
          <RegistryEntry
            type="figure"
            props={{ svg: content.svg, caption: content.description ?? "" }}
          />
        </Framed>
      );
    case "Mermaid":
      return (
        <Framed
          label={label ?? ""}
          caption={content.description ?? ""}
          description={content.description}
        >
          <RegistryEntry
            type="mermaid-diagram"
            props={{
              source: content.source,
              caption: content.description ?? "",
            }}
          />
        </Framed>
      );
    case "Check":
      return <Check questions={content.questions} {...at} />;
    case "Fallback":
      return (
        <Described label={label ?? ""} description={content.description} />
      );
    default:
      return unknownTag(content);
  }
}

/**
 * The text `range` covers, read as marks read it: a formula it touches as
 * its TeX source, a citation marker not at all.
 */
function textIn(range: Range): string {
  const container = range.commonAncestorContainer;
  const scope =
    container instanceof Element ? container : container.parentElement;
  if (scope === null) return "";
  const formulas = new Set<Element>();
  let text = "";
  const walker = document.createTreeWalker(scope, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    if (!range.intersectsNode(node) || node.parentElement === null) continue;
    if (node.parentElement.closest("[data-citation]") !== null) continue;
    const formula = node.parentElement.closest("[data-formula]");
    if (formula !== null) {
      if (!formulas.has(formula)) {
        formulas.add(formula);
        text += formula.getAttribute("data-formula");
      }
      continue;
    }
    const value = node.nodeValue ?? "";
    const from = node === range.startContainer ? range.startOffset : 0;
    const to = node === range.endContainer ? range.endOffset : value.length;
    text += value.slice(from, to);
  }
  return text;
}

/** Prose selected within one prose block of `root`; null otherwise. */
function passageIn(root: Element, selection: Selection | null): Passage | null {
  if (selection === null || selection.isCollapsed) return null;
  const blockOf = (node: Node | null) => {
    const element = node instanceof Element ? node : node?.parentElement;
    const block = element?.closest("[data-prose-block]");
    return block && root.contains(block)
      ? block.getAttribute("data-prose-block")
      : null;
  };
  const block = blockOf(selection.anchorNode);
  if (block === null || block !== blockOf(selection.focusNode)) return null;
  const text = textIn(selection.getRangeAt(0));
  return text.trim() === "" ? null : { block: Number(block), text };
}

const nothing: never[] = [];

function byBlock<T extends { block: number }>(items: T[]): Map<number, T[]> {
  const grouped = new Map<number, T[]>();
  for (const item of items) {
    grouped.set(item.block, [...(grouped.get(item.block) ?? []), item]);
  }
  return grouped;
}

/**
 * A section's blocks in order, keyed by block: prose as Markdown in
 * Typeset, visuals, figures and diagrams through the catalog's registry in
 * the figure frame with running numbers, check questions through the
 * registry (answerable unless `answerable` is false), and a placeholder of
 * fixed height for a block still arriving.
 * Every block owns only the space above it, so a block that arrives or
 * grows never moves the ones before it.
 */
export function Blocks({
  blocks,
  versionId = null,
  citations = nothing,
  sources = nothing,
  marks = nothing,
  markComponent,
  onSelection,
  answerable = true,
}: {
  blocks: ShownBlock[];
  /** The stored version shown, whose check questions can be answered. */
  versionId?: string | null;
  /** The section's citations; their markers sit where their claims end. */
  citations?: Citation[];
  sources?: Source[];
  /** Quoted passages of prose blocks, shown through `markComponent`. */
  marks?: Mark[];
  markComponent?: ComponentType<MarkProps>;
  /** Called with what a selection covers whenever the selection changes. */
  onSelection?: (passage: Passage | null) => void;
  /** False shows check questions with their options, not to be answered. */
  answerable?: boolean;
}) {
  const root = useRef<HTMLDivElement>(null);
  const labels = useMemo(() => labelsOf(blocks), [blocks]);
  const citationsByBlock = useMemo(() => byBlock(citations), [citations]);
  const marksByBlock = useMemo(() => byBlock(marks), [marks]);

  const report = useEffectEvent(() => {
    if (root.current !== null) {
      onSelection?.(passageIn(root.current, document.getSelection()));
    }
  });
  const listening = onSelection !== undefined;
  useEffect(() => {
    if (!listening) return;
    document.addEventListener("selectionchange", report);
    return () => document.removeEventListener("selectionchange", report);
  }, [listening]);

  return (
    <div ref={root} className="typeset">
      {blocks.map((block) => {
        const prose = (text: string, streaming: boolean) => (
          <Prose
            block={block.index}
            text={text}
            streaming={streaming}
            citations={citationsByBlock.get(block.index)}
            sources={sources}
            marks={marksByBlock.get(block.index)}
            markComponent={markComponent}
          />
        );
        return (
          <Fragment key={block.index}>
            {block.state === "complete" ? (
              <Complete
                content={block.content}
                label={labels.get(block.index)}
                prose={(text) => prose(text, false)}
                at={{ versionId, block: block.index, answerable }}
              />
            ) : block.kind === "Prose" ? (
              prose(block.text, true)
            ) : (
              <Placeholder kind={block.kind} />
            )}
          </Fragment>
        );
      })}
    </div>
  );
}
