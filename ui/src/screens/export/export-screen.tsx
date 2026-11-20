import { useEffect, useId, useMemo, useRef, useState } from "react";

import { useAtlasStore } from "@/app/store";
import { CitationScope } from "@/catalog/components/citation/citation";
import { describeError } from "@/core/client";
import type { ContentsEntry } from "@/core/generated/ContentsEntry";
import type { ExportDocument } from "@/core/generated/ExportDocument";
import type { ExportSection } from "@/core/generated/ExportSection";
import type { GlossaryEntry } from "@/core/generated/GlossaryEntry";
import { cn } from "@/lib/utils";
import { Blocks } from "@/reader/section/blocks/blocks";
import { Sources } from "@/reader/section/sources/sources";
import type { ShownBlock } from "@/store";
import { ThemeScope } from "@/theme/provider";

import { useExportSignal } from "./ready";

type Loaded =
  | { state: "loading" }
  | { state: "ready"; document: ExportDocument }
  | { state: "failed"; error: unknown };

/** The id a section's page carries, which the contents link to. */
const anchorOf = (nodeId: string) => `section-${nodeId}`;

/**
 * Page rules for printing: margins, no page ground, and nothing that would
 * split a figure or a table row. Links print as their text.
 */
const PRINT_RULES = `
@page { margin: 18mm 16mm; }
@media print {
  html, body { background: none !important; color-scheme: light !important; }
  [data-export-document] :is(figure, tr, [data-slot="figure"]) { break-inside: avoid; }
  [data-export-document] a { color: inherit; text-decoration: none; }
  [data-export-document] [data-slot="code-block-actions"] { display: none; }
}
`;

/**
 * The course as it is printed, for the shell to turn into a PDF: a title
 * page, the contents with every index entry, each section with a stored
 * version in index order, deep dives under their section, and the glossary
 * as an appendix. Always in the light theme, on white, with no rail, header,
 * foot or control; it signals when it is ready to print.
 */
export function ExportScreen({ courseId }: { courseId: string }) {
  const { send } = useAtlasStore();
  const [loaded, setLoaded] = useState<Loaded>({ state: "loading" });
  const root = useRef<HTMLElement>(null);

  useEffect(() => {
    let current = true;
    send({ type: "GetExportDocument", courseId }).then(
      ({ document }) => {
        if (current) setLoaded({ state: "ready", document });
      },
      (error: unknown) => {
        if (current) setLoaded({ state: "failed", error });
      },
    );
    return () => {
      current = false;
    };
  }, [send, courseId]);

  useExportSignal(root, loaded.state);

  return (
    <ThemeScope
      theme="light"
      className="min-h-svh bg-background text-foreground print:min-h-0 print:bg-transparent"
    >
      <style>{PRINT_RULES}</style>
      <main
        ref={root}
        data-export-document
        aria-busy={loaded.state === "loading"}
        className="mx-auto max-w-180 px-5 pt-24 pb-24 print:max-w-none print:px-0 print:pt-0 print:pb-0"
      >
        {loaded.state === "loading" && (
          <p role="status" className="text-13 text-muted-foreground">
            Preparing the export…
          </p>
        )}
        {loaded.state === "failed" && (
          <p role="alert" className="text-15 text-body">
            The course could not be exported. {describeError(loaded.error)}
          </p>
        )}
        {loaded.state === "ready" && <Printed document={loaded.document} />}
      </main>
    </ThemeScope>
  );
}

function Printed({ document }: { document: ExportDocument }) {
  const sections = document.contents.filter(
    (entry) => entry.level !== "Chapter",
  );
  const opened = sections.filter((entry) => entry.included).length;
  return (
    <>
      <header className="flex flex-col gap-3 pt-[22vh] pb-24 break-after-page print:pb-0">
        <h1 className="text-28 font-medium tracking-[-0.012em] text-balance text-heading">
          {document.title}
        </h1>
        <p className="text-15 text-muted-foreground tabular-nums">
          {opened} of {sections.length} sections opened
        </p>
      </header>
      <Contents entries={document.contents} />
      {document.sections.map((section) => (
        <PrintedSection key={section.nodeId} section={section} />
      ))}
      {document.glossary.length > 0 && <Glossary entries={document.glossary} />}
    </>
  );
}

interface ContentsNode {
  entry: ContentsEntry;
  children: ContentsNode[];
}

/** The flat contents as a tree: each entry under the last shallower one. */
function treeOf(entries: ContentsEntry[]): ContentsNode[] {
  const roots: ContentsNode[] = [];
  const open: ContentsNode[] = [];
  for (const entry of entries) {
    const node = { entry, children: [] };
    while ((open.at(-1)?.entry.depth ?? -1) >= entry.depth) open.pop();
    (open.at(-1)?.children ?? roots).push(node);
    open.push(node);
  }
  return roots;
}

function Contents({ entries }: { entries: ContentsEntry[] }) {
  const headingId = useId();
  const tree = useMemo(() => treeOf(entries), [entries]);
  return (
    <nav
      aria-labelledby={headingId}
      className="border-t border-edge pt-14 pb-24 break-after-page print:border-t-0 print:pt-0 print:pb-0"
    >
      <h2 id={headingId} className="text-20 font-medium text-heading">
        Contents
      </h2>
      <p className="mt-2 mb-8 text-13 text-muted-foreground">
        Sections not yet opened are listed by title only.
      </p>
      <ContentsList nodes={tree} />
    </nav>
  );
}

function ContentsList({ nodes }: { nodes: ContentsNode[] }) {
  return (
    <ol className="flex flex-col">
      {nodes.map(({ entry, children }) => (
        <li
          key={entry.nodeId}
          className={cn(
            entry.level === "Chapter"
              ? "mt-6 break-inside-avoid first:mt-0"
              : "pl-4",
          )}
        >
          <ContentsLine entry={entry} />
          {children.length > 0 && <ContentsList nodes={children} />}
        </li>
      ))}
    </ol>
  );
}

function ContentsLine({ entry }: { entry: ContentsEntry }) {
  if (entry.level === "Chapter") {
    return (
      <span className="block py-1.5 text-15 font-medium text-heading">
        {entry.title}
      </span>
    );
  }
  const size = entry.level === "DeepDive" ? "text-13" : "text-15";
  // State is a mark, as on the route line: a filled node for an opened
  // section, a hollow one for a title only.
  const node = (
    <span
      aria-hidden
      className={cn(
        "size-1.5 shrink-0 rounded-full",
        entry.included ? "bg-faint" : "border border-faint",
      )}
    />
  );
  if (!entry.included) {
    return (
      <span
        className={cn(
          "flex items-center gap-2.5 py-1 text-muted-foreground",
          size,
        )}
      >
        {node}
        {entry.title}
      </span>
    );
  }
  const anchor = anchorOf(entry.nodeId);
  return (
    <a
      href={`#${anchor}`}
      // The location's hash is the route: on screen the link scrolls to its
      // section instead of changing it; in the PDF it is an internal link.
      onClick={(event) => {
        event.preventDefault();
        document.getElementById(anchor)?.scrollIntoView();
      }}
      className={cn(
        "flex items-center gap-2.5 rounded-sm py-1 text-body outline-none hover:text-heading focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid",
        size,
      )}
    >
      {node}
      {entry.title}
    </a>
  );
}

function PrintedSection({ section }: { section: ExportSection }) {
  const titleId = useId();
  const { version } = section;
  const blocks = useMemo<ShownBlock[]>(
    () =>
      version.blocks.map((block) => ({
        state: "complete",
        index: block.index,
        content: block.content,
      })),
    [version.blocks],
  );
  const broad = section.kind === "BroadPass";
  const Heading = broad ? "h2" : "h3";
  return (
    <article
      id={anchorOf(section.nodeId)}
      aria-labelledby={titleId}
      className={cn(
        broad
          ? "border-t border-edge pt-14 pb-16 break-before-page print:border-t-0 print:pt-0"
          : "pb-16",
      )}
    >
      <header className={broad ? "mb-7" : "mb-6"}>
        <Heading
          id={titleId}
          className={cn(
            "font-medium text-balance text-heading",
            broad ? "text-28" : "text-20",
          )}
        >
          {section.title}
        </Heading>
        <p
          className={cn(
            "mt-2 text-muted-foreground",
            broad ? "text-16" : "text-15",
          )}
        >
          {section.summary}
        </p>
      </header>
      <CitationScope>
        <Blocks
          blocks={blocks}
          versionId={version.id}
          citations={version.citations}
          sources={version.sources}
          answerable={false}
        />
        <Sources sources={version.sources} />
      </CitationScope>
    </article>
  );
}

function Glossary({ entries }: { entries: GlossaryEntry[] }) {
  const headingId = useId();
  return (
    <section
      aria-labelledby={headingId}
      className="border-t border-edge pt-14 break-before-page print:border-t-0 print:pt-0"
    >
      <h2 id={headingId} className="mb-6 text-28 font-medium text-heading">
        Glossary
      </h2>
      <dl>
        {entries.map((entry) => (
          <div
            key={entry.id}
            className="border-t border-edge py-4 break-inside-avoid"
          >
            <dt className="text-16 font-medium text-heading">{entry.term}</dt>
            <dd className="mt-1 text-16 leading-[1.65] text-body">
              {entry.definition}
            </dd>
            <dd className="mt-1.5 text-13 text-muted-foreground">
              From {entry.sectionTitle}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
