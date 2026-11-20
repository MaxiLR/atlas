import "katex/dist/katex.css";

import type { Element } from "hast";
import {
  memo,
  useMemo,
  type ComponentProps,
  type ComponentType,
  type ReactNode,
} from "react";
import Markdown, { type Components, type Options } from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";

import { failureInStore, useAtlasStore } from "@/app/store";
import {
  CodeBlock,
  CodeBlockActions,
  CodeBlockContent,
  CodeBlockCopyButton,
} from "@/components/ui/code-block";
import type { Citation } from "@/core/generated/Citation";
import type { Source } from "@/core/generated/Source";

import {
  rehypeFormulas,
  rehypeMarks,
  rehypeProseBlock,
  remarkCitations,
  type Mark,
} from "./markdown-plugins";
import { RegistryEntry } from "./registry-entry";

export interface MarkProps {
  id: string;
  kind: string;
  /** The marked text: a part of the quote when it crosses inline formatting. */
  children: ReactNode;
}

const ACTIVE = /^https?:\/\//i;

/**
 * A link the model wrote. Only an http or https link is active, and
 * activating it asks the core to open it in the system browser; the webview
 * never navigates.
 */
function Link({ href, children }: ComponentProps<"a">) {
  const { send } = useAtlasStore();
  if (href === undefined || !ACTIVE.test(href)) return <span>{children}</span>;
  return (
    <a
      href={href}
      onClick={(event) => {
        event.preventDefault();
        send({ type: "OpenExternal", url: href }).catch(failureInStore);
      }}
      onAuxClick={(event) => event.preventDefault()}
    >
      {children}
    </a>
  );
}

/** An image is never loaded: its description stands in its place. */
function ImageText({ alt }: ComponentProps<"img">) {
  return alt ? <span>{alt}</span> : null;
}

function textOf(node: Element | Element["children"][number]): string {
  if (node.type === "text") return node.value;
  if ("children" in node) return node.children.map(textOf).join("");
  return "";
}

/** The prose block a top-level element names, as the plugin set it. */
function blockOf(props: object): string | undefined {
  const block = (props as Record<string, unknown>)["data-prose-block"];
  return block === undefined ? undefined : String(block);
}

type FromMarkdown<Tag extends "pre" | "table" | "h1"> = ComponentProps<Tag> & {
  node?: Element;
};

/** A fenced code block, in the Code Block component. */
function Code(props: FromMarkdown<"pre">) {
  const code =
    props.node === undefined ? "" : textOf(props.node).replace(/\n$/, "");
  return (
    <CodeBlock data-prose-block={blockOf(props)}>
      <CodeBlockActions>
        <CodeBlockCopyButton value={code} />
      </CodeBlockActions>
      <CodeBlockContent>{code}</CodeBlockContent>
    </CodeBlock>
  );
}

/** A table scrolls sideways instead of squeezing its columns. */
function Table(props: FromMarkdown<"table">) {
  return (
    <div className="typeset-scroll" data-prose-block={blockOf(props)}>
      <table>{props.children}</table>
    </div>
  );
}

/** The section's title is its only first-level heading. */
function Heading(props: FromMarkdown<"h1">) {
  return <h2 data-prose-block={blockOf(props)}>{props.children}</h2>;
}

/** The default look of a mark: highlighted text. */
function Highlight({ children }: MarkProps) {
  return <mark>{children}</mark>;
}

const KATEX = { strict: "ignore", maxSize: 20, maxExpand: 200 } as const;
const nothing: never[] = [];

/**
 * A prose block as Markdown in Typeset: GFM, KaTeX maths, code blocks,
 * links through the system browser and no raw HTML. Its top-level elements
 * name the block, citation markers sit where their claims end, and marks
 * wrap their quoted text through `markComponent`.
 */
export const Prose = memo(function Prose({
  block,
  text,
  streaming,
  citations = nothing,
  sources = nothing,
  marks = nothing,
  markComponent: MarkComponent = Highlight,
}: {
  block: number;
  text: string;
  streaming: boolean;
  citations?: Citation[];
  sources?: Source[];
  marks?: Mark[];
  markComponent?: ComponentType<MarkProps>;
}) {
  const remarkPlugins = useMemo<Options["remarkPlugins"]>(
    () => [remarkGfm, remarkMath, [remarkCitations, { citations }]],
    [citations],
  );
  const rehypePlugins = useMemo<Options["rehypePlugins"]>(
    () => [
      [rehypeKatex, KATEX],
      [rehypeFormulas, { streaming }],
      [rehypeMarks, { marks }],
      [rehypeProseBlock, { block }],
    ],
    [streaming, marks, block],
  );
  const components = useMemo<Components>(
    () => ({
      a: Link,
      img: ImageText,
      pre: Code,
      table: Table,
      h1: Heading,
      cite: ({ node }: { node?: Element }) => {
        const source = sources.find(
          ({ number }) => number === Number(node?.properties.dataSource),
        );
        return source === undefined ? null : (
          <span data-citation="">
            <RegistryEntry type="citation" props={{ ...source }} />
          </span>
        );
      },
      mark: ({ node, children }: { node?: Element; children?: ReactNode }) => (
        <MarkComponent
          id={String(node?.properties.dataMarkId)}
          kind={String(node?.properties.dataMarkKind)}
        >
          {children}
        </MarkComponent>
      ),
    }),
    [sources, MarkComponent],
  );

  return (
    <Markdown
      skipHtml
      remarkPlugins={remarkPlugins}
      rehypePlugins={rehypePlugins}
      components={components}
    >
      {text}
    </Markdown>
  );
});
