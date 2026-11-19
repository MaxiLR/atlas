import type { BlockContent } from "@/core/generated/BlockContent";
import type { BlockKind } from "@/core/generated/BlockKind";
import type { ShownBlock } from "@/store";
import { unknownTag } from "@/store/unknown-tag";

/** Prose as paragraphs, split at blank lines. */
function Prose({ text }: { text: string }) {
  return text
    .split(/\n{2,}/)
    .filter((paragraph) => paragraph.trim() !== "")
    .map((paragraph, at) => <p key={at}>{paragraph}</p>);
}

/** One line of text standing in for a block that is not prose. */
function TextLine({ label, text }: { label: string; text: string }) {
  return (
    <p className="flex items-baseline gap-3.5 rounded-lg border border-edge px-4 py-2.5 text-13 text-muted-foreground">
      <span className="font-mono text-12 tracking-[0.04em] whitespace-nowrap uppercase">
        {label}
      </span>
      {text}
    </p>
  );
}

const kindLabels: Record<BlockKind, string> = {
  Prose: "Text",
  Visual: "Visual",
  Figure: "Figure",
  Mermaid: "Diagram",
  Check: "Check",
};

function Complete({ content }: { content: BlockContent }) {
  switch (content.type) {
    case "Prose":
      return <Prose text={content.text} />;
    case "Visual":
    case "Figure":
    case "Mermaid":
      return (
        <TextLine
          label={kindLabels[content.type]}
          text={content.description ?? "No description given."}
        />
      );
    case "Check":
      return (
        <TextLine
          label={kindLabels.Check}
          text={
            content.questions.length === 1
              ? "A check question."
              : `${content.questions.length} check questions.`
          }
        />
      );
    case "Fallback":
      return <TextLine label="Figure" text={content.description} />;
    default:
      return unknownTag(content);
  }
}

/**
 * A section's blocks in order: prose as text, and a line of text for every
 * other block, whether complete or still arriving.
 */
export function Blocks({ blocks }: { blocks: ShownBlock[] }) {
  return (
    <div className="typeset">
      {blocks.map((block) =>
        block.state === "complete" ? (
          <Complete key={block.index} content={block.content} />
        ) : block.kind === "Prose" ? (
          <Prose key={block.index} text={block.text} />
        ) : (
          <TextLine
            key={block.index}
            label={kindLabels[block.kind]}
            text="Arriving…"
          />
        ),
      )}
    </div>
  );
}
