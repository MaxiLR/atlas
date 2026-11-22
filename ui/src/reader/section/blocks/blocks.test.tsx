import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { AtlasStoreProvider, useAtlas } from "@/app/store";
import { CatalogVisual } from "@/catalog/catalog-visual";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import type { Block } from "@/core/generated/Block";
import type { BlockContent } from "@/core/generated/BlockContent";
import type { BlockKind } from "@/core/generated/BlockKind";
import type { CheckResult } from "@/core/generated/CheckResult";
import type { Citation } from "@/core/generated/Citation";
import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { Source } from "@/core/generated/Source";
import { selectSection } from "@/store";
import { drained } from "@/test/fake-core";

import { Blocks, type Mark, type MarkProps, type Passage } from "./blocks";

const SECTION = "section-1";

const comparison = {
  root: "table",
  elements: {
    table: {
      type: "comparison",
      props: {
        caption: "The same collision, three outcomes",
        columns: ["Ocean meets continent", "Ocean meets ocean"],
        rows: [
          {
            label: "What sinks",
            cells: ["The oceanic plate", "The older, colder plate"],
          },
        ],
      },
      children: [],
    },
  },
};

const callout = {
  root: "note",
  elements: {
    note: {
      type: "callout",
      props: {
        title: "Convergent does not mean a crash",
        body: "Nothing collides at speed: plates move about as fast as fingernails grow.",
      },
      children: [],
    },
  },
};

const figureSvg =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0H10" stroke="currentColor"/></svg>';

/**
 * The block list as the reading column shows it, fed by a store connected
 * to a client whose events the test sends. `requests` records what the
 * interface asked of the core.
 */
function renderBlocks(
  options: {
    marks?: Mark[];
    markComponent?: (props: MarkProps) => ReactNode;
    onSelection?: (passage: Passage | null) => void;
  } = {},
) {
  const listeners = new Set<(event: CoreEvent) => void>();
  const requests: CoreRequest[] = [];
  const client: CoreClient = {
    call: (async (request: CoreRequest) => {
      requests.push(request);
      return { type: "Done" };
    }) as CoreClient["call"],
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };

  function Shown() {
    const display = useAtlas((state) => selectSection(state, SECTION));
    if (display === null) return null;
    return (
      <Blocks
        blocks={display.blocks}
        citations={display.citations}
        sources={display.sources}
        marks={options.marks}
        markComponent={options.markComponent}
        onSelection={options.onSelection}
      />
    );
  }

  const view = render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <main data-testid="column">
          <Shown />
        </main>
      </AtlasStoreProvider>
    </CoreProvider>,
  );

  const generate = (event: GenerationEvent) =>
    act(() => {
      for (const listener of listeners) {
        listener({
          type: "Generation",
          generationId: "generation-1",
          target: {
            type: "Section",
            courseId: "course-1",
            sectionId: SECTION,
            style: "Original",
          },
          event,
        });
      }
    });
  generate({ type: "Started" });

  return {
    ...view,
    requests,
    column: screen.getByTestId("column"),
    generate,
    open: (block: number, kind: BlockKind) =>
      generate({ type: "BlockOpened", block, kind }),
    delta: (block: number, text: string) =>
      generate({ type: "TextDelta", block, text }),
    complete: (index: number, content: BlockContent) =>
      generate({ type: "BlockCompleted", block: { index, content } }),
    /** A section whose blocks all arrived, in order. */
    section(contents: BlockContent[]) {
      contents.forEach((content, index) => {
        const block: Block = { index, content };
        generate({ type: "BlockCompleted", block });
      });
    },
    cite: (citation: Citation, source: Source) =>
      generate({ type: "CitationAdded", citation, source }),
  };
}

const prose = (text: string): BlockContent => ({ type: "Prose", text });

describe("the block list", () => {
  it("renders prose through Markdown: headings, emphasis, inline code, lists and tables", () => {
    const { section, column } = renderBlocks();
    section([
      prose(
        [
          "## Three kinds of convergence",
          "What forms depends on **what the plates carry**, as `d = v t` says.",
          "- Ocean meets continent\n- Ocean meets ocean",
          "1. The slab sinks.\n2. Water rises.",
          "| Boundary | What sinks |\n| --- | --- |\n| Ocean and continent | The oceanic plate |",
        ].join("\n\n"),
      ),
    ]);

    expect(
      screen.getByRole("heading", {
        level: 2,
        name: "Three kinds of convergence",
      }),
    ).toBeVisible();
    expect(screen.getByText("what the plates carry").tagName).toBe("STRONG");
    expect(screen.getByText("d = v t").tagName).toBe("CODE");
    const lists = screen.getAllByRole("list");
    expect(lists).toHaveLength(2);
    expect(within(lists[0]).getAllByRole("listitem")).toHaveLength(2);
    const table = screen.getByRole("table");
    expect(
      within(table)
        .getAllByRole("columnheader")
        .map((cell) => cell.textContent),
    ).toEqual(["Boundary", "What sinks"]);
    expect(within(table).getByRole("cell", { name: "The oceanic plate" }));
    expect(column).not.toHaveTextContent("##");
    expect(column).not.toHaveTextContent("**");
  });

  it("renders inline and display maths with KaTeX", () => {
    const { section, column } = renderBlocks();
    section([
      prose(
        "A plate covers $d = vt$ in a year.\n\n$$\nT^2 = \\frac{4\\pi^2}{GM} a^3\n$$",
      ),
    ]);

    const formulas = column.querySelectorAll("math");
    expect(formulas).toHaveLength(2);
    // MathML elements are not HTML elements, which jest-dom's attribute
    // matcher requires, so the attribute is read directly.
    expect(formulas[0].getAttribute("display")).not.toBe("block");
    expect(formulas[0]).toHaveTextContent("d=vt");
    expect(formulas[1].getAttribute("display")).toBe("block");
    expect(formulas[1].querySelector("annotation")).toHaveTextContent(
      "T^2 = \\frac{4\\pi^2}{GM} a^3",
    );
    expect(column).not.toHaveTextContent("$");
  });

  it("says when a formula is invalid and shows its source instead", () => {
    const { section, column } = renderBlocks();
    section([prose("Inline $\\sqrt{2$ here.\n\n$$\n\\frac{1}{\n$$")]);

    expect(column.querySelectorAll("math")).toHaveLength(0);
    const errors = screen.getAllByText(/^This formula could not be shown/);
    expect(errors).toHaveLength(2);
    expect(column).toHaveTextContent("\\sqrt{2");
    expect(column).toHaveTextContent("\\frac{1}{");
  });

  it("renders a fenced code block with the Code Block component", () => {
    const { section } = renderBlocks();
    section([prose("```python\nspeed = distance / time\nprint(speed)\n```")]);

    const code = screen.getByText(/speed = distance \/ time/);
    const block = code.closest("pre");
    expect(block).not.toBeNull();
    expect(block).toHaveTextContent("print(speed)");
    // The code scrolls on its own, so a keyboard can reach it.
    expect(block).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("button", { name: "Copy" })).toBeVisible();
  });

  it("frames figures, diagrams, tables and fallbacks with their own running numbers and captions", () => {
    const { section } = renderBlocks();
    section([
      prose("The plate bends."),
      {
        type: "Figure",
        svg: figureSvg,
        description: "An oceanic plate subducting. Not to scale.",
      },
      {
        type: "Visual",
        spec: comparison,
        description: "Three outcomes of one collision.",
      },
      {
        type: "Mermaid",
        source: "flowchart LR\n  A[Trench] --> B[Arc]",
        description: "From the trench to the arc.",
      },
      { type: "Fallback", description: "A timeline of the collision." },
      { type: "Visual", spec: callout, description: "A key idea." },
      {
        type: "Visual",
        spec: { root: "x", elements: {} },
        description: "A visual that is not in the catalog.",
      },
    ]);

    const figures = screen.getAllByRole("figure");
    expect(
      figures.map((figure) => figure.querySelector("figcaption")?.textContent),
    ).toEqual([
      "Fig. 1 An oceanic plate subducting. Not to scale.",
      "Table 1 The same collision, three outcomes",
      "Fig. 2 From the trench to the arc.",
      "Fig. 3 A timeline of the collision.",
      "Fig. 4 A visual that is not in the catalog.",
    ]);
    expect(
      screen.getByRole("figure", {
        name: "Fig. 1 An oceanic plate subducting. Not to scale.",
      }),
    ).toBe(figures[0]);

    // The frame names the figure; the component inside shows its content
    // only, so the caption is shown once.
    expect(
      within(figures[0]).getAllByText(
        "An oceanic plate subducting. Not to scale.",
      ),
    ).toHaveLength(1);
    expect(within(figures[1]).queryByRole("heading")).toBeNull();
    expect(
      within(figures[1]).getAllByText("The same collision, three outcomes"),
    ).toHaveLength(1);
    // The comparison is drawn through the registry inside its frame.
    expect(figures[1]).toHaveTextContent("Ocean meets continent");
    expect(figures[1]).toHaveTextContent("The older, colder plate");

    // A callout is not framed and takes no number.
    const point = screen.getByText(/Nothing collides at speed/);
    expect(point.closest("figure")).toBeNull();
    expect(screen.getByText("Convergent does not mean a crash")).toBeVisible();
  });

  it("renders check questions through the registry under their heading, unframed", () => {
    const { section } = renderBlocks();
    section([
      prose("Before the question."),
      {
        type: "Check",
        questions: [
          {
            index: 0,
            stem: "Which plate sinks when old ocean floor meets young?",
            options: [
              "The old plate",
              "The young plate",
              "Neither",
              "Both at once",
            ],
            answer: null,
          },
        ],
      },
    ]);

    expect(
      screen.getByRole("heading", { level: 2, name: "Check question" }),
    ).toBeVisible();
    const stem = screen.getByText(
      "Which plate sinks when old ocean floor meets young?",
    );
    expect(stem.closest("figure")).toBeNull();
    for (const option of [
      "The old plate",
      "The young plate",
      "Neither",
      "Both at once",
    ]) {
      expect(screen.getByText(option)).toBeVisible();
    }
    expect(screen.queryByRole("figure")).toBeNull();
  });

  it("holds an opened block's place with a placeholder that the block replaces in place", () => {
    const { open, delta, complete, column } = renderBlocks();
    open(0, "Prose");
    delta(0, "The first paragraph.");
    complete(0, prose("The first paragraph."));
    open(1, "Visual");
    const placeholder = screen.getByText(/^Drawing the visual/);
    const shownBefore = placeholder.textContent;
    delta(1, '{"root":"table","elements":{');
    // The streamed spec is never shown and the placeholder does not change.
    expect(column).not.toHaveTextContent('"root"');
    expect(screen.getByText(/^Drawing the visual/).textContent).toBe(
      shownBefore,
    );

    open(2, "Prose");
    delta(2, "The paragraph after the visual.");
    const first = screen.getByText("The first paragraph.");
    const following = screen.getByText("The paragraph after the visual.");
    expect(
      placeholder.compareDocumentPosition(following) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    complete(1, {
      type: "Visual",
      spec: comparison,
      description: "Three outcomes.",
    });

    expect(screen.queryByText(/^Drawing the visual/)).toBeNull();
    const figure = screen.getByRole("figure");
    // The blocks around it are the same nodes, in the same order.
    expect(screen.getByText("The first paragraph.")).toBe(first);
    expect(screen.getByText("The paragraph after the visual.")).toBe(following);
    expect(
      first.compareDocumentPosition(figure) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      figure.compareDocumentPosition(following) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    delta(2, " More text arrives.");
    expect(screen.getByText("The first paragraph.")).toBe(first);
    expect(screen.getByRole("figure")).toBe(figure);
  });

  it("names the kind of block each placeholder stands for", () => {
    const { open } = renderBlocks();
    open(0, "Figure");
    open(1, "Mermaid");
    open(2, "Check");
    expect(screen.getByText(/^Drawing the figure/)).toBeVisible();
    expect(screen.getByText(/^Drawing the diagram/)).toBeVisible();
    expect(screen.getByText(/^Writing the check question/)).toBeVisible();
  });

  it("shows long content in full: many paragraphs, an unbroken word, a long code line and a long caption", () => {
    const { section, column } = renderBlocks();
    const paragraphs = Array.from(
      { length: 60 },
      (_, at) => `Paragraph ${at + 1} of the lesson.`,
    );
    const word = "pneumono".repeat(50);
    const line = `const values = [${Array.from({ length: 80 }, (_, at) => at).join(", ")}];`;
    const caption = `A long caption. ${"It goes on. ".repeat(40)}`.trim();
    section([
      prose([...paragraphs, word, "```js\n" + line + "\n```"].join("\n\n")),
      { type: "Fallback", description: caption },
    ]);

    expect(screen.getByText("Paragraph 1 of the lesson.")).toBeVisible();
    expect(screen.getByText("Paragraph 60 of the lesson.")).toBeVisible();
    expect(screen.getByText(word)).toBeVisible();
    expect(column).toHaveTextContent(line);
    expect(screen.getByText(line).closest("pre")).toHaveAttribute(
      "tabindex",
      "0",
    );
    expect(
      screen.getByRole("figure").querySelector("figcaption"),
    ).toHaveTextContent(`Fig. 1 ${caption}`);
  });

  it("places a citation marker at its offset through the registry's citation entry", () => {
    const { section, cite, column } = renderBlocks();
    const text = "Plates move a few centimetres a year. That is slow.";
    section([prose(text)]);
    const source: Source = {
      number: 1,
      url: "https://pubs.usgs.gov/gip/dynamic/understanding.html",
      title: "Understanding plate motions",
    };
    const start = text.indexOf("a few centimetres a year");
    cite(
      {
        block: 0,
        start,
        end: start + "a few centimetres a year".length,
        source: 1,
      },
      source,
    );

    const { container: entry } = render(
      <CatalogVisual
        spec={{
          root: "c",
          elements: {
            c: { type: "citation", props: { ...source }, children: [] },
          },
        }}
      />,
    );
    const marker = entry.textContent ?? "";
    expect(marker).toContain("1");
    const paragraph = within(column).getByText(/^Plates move/);
    expect(paragraph.textContent).toBe(
      `Plates move a few centimetres a year${marker}. That is slow.`,
    );
  });

  it("places a citation marker after a formula or inline code it ends on, and counts entities and escapes as the source writes them", () => {
    const { section, cite, column } = renderBlocks();
    const texts = [
      "Energy equals $E=mc^2$. Then more.",
      "The rate is `v`. Slow.",
      "It costs AT&amp;T a lot. More.",
      "Stars \\*shine\\* at night. Dark.",
    ];
    section(texts.map(prose));
    const source: Source = {
      number: 1,
      url: "https://pubs.usgs.gov/gip/dynamic/understanding.html",
      title: "Understanding plate motions",
    };
    const ends = [". Then more.", ". Slow.", ". More.", ". Dark."];
    texts.forEach((text, block) => {
      const start = text.indexOf(" ") + 1;
      cite(
        { block, start, end: text.indexOf(ends[block]!), source: 1 },
        source,
      );
    });

    const { container: entry } = render(
      <CatalogVisual
        spec={{
          root: "c",
          elements: {
            c: { type: "citation", props: { ...source }, children: [] },
          },
        }}
      />,
    );
    const marker = entry.textContent ?? "";
    const paragraphs = column.querySelectorAll("p");
    expect(paragraphs).toHaveLength(4);

    // The formula comes before the marker, and only the sentence's end after it.
    const formula = paragraphs[0]!.querySelector("math")!;
    const cited = within(paragraphs[0]!).getByText(source.title);
    expect(
      formula.compareDocumentPosition(cited) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    const after = document.createRange();
    after.setStartAfter(cited);
    after.setEnd(paragraphs[0]!, paragraphs[0]!.childNodes.length);
    expect(after.toString()).toBe(". Then more.");

    expect(paragraphs[1]!.textContent).toBe(`The rate is v${marker}. Slow.`);
    expect(paragraphs[2]!.textContent).toBe(
      `It costs AT&T a lot${marker}. More.`,
    );
    expect(paragraphs[3]!.textContent).toBe(
      `Stars *shine* at night${marker}. Dark.`,
    );
  });

  it("counts a citation's offsets in Unicode characters, as the core does, so characters beyond the BMP before a claim leave its marker in place", () => {
    const { section, cite, column } = renderBlocks();
    const texts = [
      "🌋 Mount St. Helens erupted in 1980, killing 57 people. It was the deadliest.",
      "𝑥 and 𝔽 come first, then the claim. After.",
    ];
    section(texts.map(prose));
    const source: Source = {
      number: 1,
      url: "https://pubs.usgs.gov/gip/dynamic/msh.html",
      title: "Mount St. Helens",
    };
    // Counted in Unicode characters: each of 🌋, 𝑥 and 𝔽 is one, though two
    // UTF-16 units. The first claim ends after "people", the second after
    // "claim".
    cite({ block: 0, start: 2, end: 53, source: 1 }, source);
    cite({ block: 1, start: 0, end: 34, source: 1 }, source);

    const { container: entry } = render(
      <CatalogVisual
        spec={{
          root: "c",
          elements: {
            c: { type: "citation", props: { ...source }, children: [] },
          },
        }}
      />,
    );
    const marker = entry.textContent ?? "";
    const paragraphs = column.querySelectorAll("p");
    expect(paragraphs[0]!.textContent).toBe(
      `🌋 Mount St. Helens erupted in 1980, killing 57 people${marker}. It was the deadliest.`,
    );
    expect(paragraphs[1]!.textContent).toBe(
      `𝑥 and 𝔽 come first, then the claim${marker}. After.`,
    );
  });
});

describe("Markdown from the model", () => {
  const hostile = [
    "<script>alert(1)</script>",
    '<img src="x" onerror="alert(1)">',
    '<iframe src="https://evil.example/"></iframe>',
    "<style>body { display: none }</style>",
    "[script link](javascript:alert(1)) [data link](data:text/html;base64,PHNjcmlwdD4=) [entity link](&#106;avascript:alert(1)) [local link](/settings)",
    "![tracker](https://evil.example/pixel.png)",
    '&lt;b&gt;escaped&lt;/b&gt; &amp; <b>raw bold</b> <a href="https://evil.example/" onclick="steal()">raw link</a>',
  ].join("\n\n");

  it("never turns HTML in the source into elements or active links", () => {
    const { section, column } = renderBlocks();
    section([prose(hostile)]);

    expect(
      column.querySelectorAll(
        "script, img, iframe, style, b, object, embed, form, input, a",
      ),
    ).toHaveLength(0);
    for (const element of column.querySelectorAll("*")) {
      for (const attribute of element.getAttributeNames()) {
        expect(attribute).not.toMatch(/^on/);
        expect(["src", "srcdoc", "style", "href"]).not.toContain(attribute);
      }
    }
    expect(screen.queryAllByRole("link")).toHaveLength(0);
    expect(screen.queryAllByRole("img")).toHaveLength(0);
    for (const label of [
      "script link",
      "data link",
      "entity link",
      "local link",
    ]) {
      expect(screen.getByText(label)).toBeVisible();
    }
    // Entities are text; tags are shown as text or dropped.
    expect(column).toHaveTextContent("<b>escaped</b> &");
    expect(column).toHaveTextContent("raw bold");
  });

  it("opens an http or https link in the system browser instead of navigating", async () => {
    const { section, requests } = renderBlocks();
    section([
      prose(
        "Read [the survey](https://www.usgs.gov/plates) or <http://example.org/a>.",
      ),
    ]);
    const before = window.location.href;

    const link = screen.getByRole("link", { name: "the survey" });
    // A click whose default is prevented returns false.
    expect(fireEvent.click(link)).toBe(false);
    expect(requests).toEqual([
      { type: "OpenExternal", url: "https://www.usgs.gov/plates" },
    ]);

    const user = userEvent.setup();
    screen.getByRole("link", { name: "http://example.org/a" }).focus();
    await user.keyboard("{Enter}");
    expect(requests).toContainEqual({
      type: "OpenExternal",
      url: "http://example.org/a",
    });
    expect(window.location.href).toBe(before);
  });
});

describe("marks and selections", () => {
  function Underline({ id, kind, children }: MarkProps) {
    return (
      <span data-testid={`mark-${id}`} data-kind={kind}>
        {children}
      </span>
    );
  }

  it("wraps exactly the quoted text through the supplied component, also across inline formatting", () => {
    const { section } = renderBlocks({
      marks: [
        {
          id: "term-1",
          kind: "term",
          block: 1,
          quote: "move at a few centimetres a year",
        },
        {
          id: "chat-1",
          kind: "passage",
          block: 0,
          quote: "nowhere in the text",
        },
      ],
      markComponent: Underline,
    });
    section([
      prose("Plates move at a few centimetres a year in the first block."),
      prose("Plates *move* at **a few centimetres** a year, as nails grow."),
    ]);

    const parts = screen.getAllByTestId("mark-term-1");
    expect(parts.map((part) => part.textContent).join("")).toBe(
      "move at a few centimetres a year",
    );
    for (const part of parts) {
      expect(part).toHaveAttribute("data-kind", "term");
      expect(screen.getByText(/in the first block/)).not.toContainElement(part);
    }
    // The formatting stays: part of the mark is inside the strong text.
    expect(parts.some((part) => part.closest("strong") !== null)).toBeTruthy();
    expect(parts.some((part) => part.closest("em") !== null)).toBeTruthy();
    expect(screen.queryByTestId("mark-chat-1")).toBeNull();
    expect(screen.getByText(/as nails grow/).textContent).toBe(
      "Plates move at a few centimetres a year, as nails grow.",
    );
  });

  it("reports the block and text a selection covers, and nothing for one across blocks", async () => {
    const onSelection = vi.fn();
    const { section } = renderBlocks({ onSelection });
    section([
      prose("The first block is about plates."),
      prose("The second block is about **subduction** and trenches."),
    ]);

    const selection = window.getSelection()!;
    const second = screen.getByText(/The second block/);
    const range = document.createRange();
    range.setStart(second.firstChild!, "The second block is about ".length);
    range.setEnd(second.lastChild!, " and trenches".length);
    await act(async () => {
      selection.removeAllRanges();
      selection.addRange(range);
      await drained();
    });
    expect(onSelection).toHaveBeenLastCalledWith({
      block: 1,
      text: "subduction and trenches",
    });

    const across = document.createRange();
    across.setStart(screen.getByText(/The first block/).firstChild!, 4);
    across.setEnd(second.firstChild!, 10);
    await act(async () => {
      selection.removeAllRanges();
      selection.addRange(across);
      await drained();
    });
    expect(onSelection).toHaveBeenLastCalledWith(null);
  });

  it("reports a selection across a citation marker and a formula as text that, given back as a mark, marks the same passage", async () => {
    const text = "A plate covers $d = vt$ in a year. That is slow.";
    const source: Source = {
      number: 1,
      url: "https://pubs.usgs.gov/gip/dynamic/understanding.html",
      title: "Understanding plate motions",
    };
    const citation: Citation = {
      block: 0,
      start: 2,
      end: text.indexOf(". That"),
      source: 1,
    };
    const onSelection = vi.fn();
    const selecting = renderBlocks({ onSelection });
    selecting.section([prose(text)]);
    selecting.cite(citation, source);

    const paragraph = selecting.column.querySelector("p")!;
    const texts = [...paragraph.childNodes].filter(
      (node) => node.nodeType === Node.TEXT_NODE,
    );
    const range = document.createRange();
    range.setStart(texts[0]!, "A plate ".length);
    range.setEnd(texts.at(-1)!, ". That".length);
    const selection = window.getSelection()!;
    await act(async () => {
      selection.removeAllRanges();
      selection.addRange(range);
      await drained();
    });
    const passage = onSelection.mock.lastCall?.[0] as Passage | null;
    expect(passage).toEqual({
      block: 0,
      text: "covers d = vt in a year. That",
    });
    selecting.unmount();

    const marking = renderBlocks({
      marks: [
        { id: "chat-1", kind: "passage", block: 0, quote: passage!.text },
      ],
      markComponent: Underline,
    });
    marking.section([prose(text)]);
    marking.cite(citation, source);

    const parts = screen.getAllByTestId("mark-chat-1");
    expect(parts[0]!.textContent).toBe("covers ");
    expect(parts.some((part) => part.querySelector("math") !== null)).toBe(
      true,
    );
    expect(parts.at(-1)!.textContent).toBe(". That");
    const cited = screen.getByText(source.title);
    for (const part of parts) expect(part).not.toContainElement(cited);
  });
});

describe("the check block", () => {
  const FIRST = "Which plate sinks when old ocean floor meets young?";
  const SECOND = "Why do two continents build mountains, not a trench?";
  const firstOptions = [
    "The old plate",
    "The young plate",
    "Neither",
    "Both at once",
  ];
  const secondOptions = [
    "Continental crust is too buoyant to sink",
    "Continents move faster",
    "Melt rises through them",
    "The mantle is too cold",
  ];
  const answered: CheckResult = {
    choice: 1,
    correct: false,
    key: 0,
    explanation: "Older ocean floor is colder and denser.",
  };

  /** A check block of the two questions, each answered or not. */
  function twoQuestions(
    first: CheckResult | null = null,
    second: CheckResult | null = null,
  ): BlockContent {
    return {
      type: "Check",
      questions: [
        { index: 0, stem: FIRST, options: firstOptions, answer: first },
        { index: 1, stem: SECOND, options: secondOptions, answer: second },
      ],
    };
  }

  const previous = () =>
    screen.getByRole("button", { name: "Previous question" });
  const next = () => screen.getByRole("button", { name: "Next question" });
  const radio = (stem: string, text: string) =>
    within(screen.getByRole("group", { name: stem })).getByRole("radio", {
      name: text,
    });

  it("shows one of two questions at a time, with its position and arrows that stop at the ends", async () => {
    const user = userEvent.setup();
    const { section } = renderBlocks();
    section([prose("Before the questions."), twoQuestions()]);

    expect(
      screen.getByRole("heading", { level: 2, name: "Check questions" }),
    ).toBeVisible();
    expect(screen.getByRole("group", { name: FIRST })).toBeVisible();
    expect(screen.queryByRole("group", { name: SECOND })).toBeNull();
    expect(screen.getByText("1 of 2")).toBeVisible();
    expect(previous()).toHaveAttribute("aria-disabled", "true");
    expect(next()).not.toHaveAttribute("aria-disabled", "true");

    // Previous at the first question does nothing.
    await user.click(previous());
    expect(screen.getByText("1 of 2")).toBeVisible();
    expect(screen.getByRole("group", { name: FIRST })).toBeVisible();

    await user.click(next());

    expect(screen.getByRole("group", { name: SECOND })).toBeVisible();
    expect(screen.queryByRole("group", { name: FIRST })).toBeNull();
    expect(screen.getByText("2 of 2")).toBeVisible();
    expect(next()).toHaveAttribute("aria-disabled", "true");
    expect(previous()).not.toHaveAttribute("aria-disabled", "true");
    // At the end, Next keeps focus and stays on the last question.
    expect(next()).toHaveFocus();
    await user.click(next());
    expect(screen.getByText("2 of 2")).toBeVisible();
    expect(next()).toHaveFocus();

    await user.click(previous());
    expect(screen.getByRole("group", { name: FIRST })).toBeVisible();
    expect(screen.getByText("1 of 2")).toBeVisible();
  });

  it("keeps the question not shown out of reach of Tab", async () => {
    const user = userEvent.setup();
    const { section } = renderBlocks();
    section([twoQuestions()]);
    const hidden = screen
      .getByRole("group", { name: SECOND, hidden: true })
      .closest("form")!;

    const reached: Element[] = [];
    for (let step = 0; step < 12; step += 1) {
      await user.tab();
      if (document.activeElement !== null) reached.push(document.activeElement);
    }

    expect(reached).toContain(previous());
    expect(reached).toContain(next());
    expect(reached).toContain(radio(FIRST, "The old plate"));
    expect(reached.filter((element) => hidden.contains(element))).toEqual([]);
  });

  it("keeps each question's choice when moving away and back", async () => {
    const user = userEvent.setup();
    const { section } = renderBlocks();
    section([twoQuestions()]);

    await user.click(radio(FIRST, "Neither"));
    await user.click(next());
    await user.click(radio(SECOND, "Continents move faster"));
    await user.click(previous());

    expect(radio(FIRST, "Neither")).toBeChecked();
    await user.click(next());
    expect(radio(SECOND, "Continents move faster")).toBeChecked();
  });

  it("changes the choice, not the question, with the arrow keys in the option group", async () => {
    const user = userEvent.setup();
    const { section } = renderBlocks();
    section([twoQuestions()]);

    await user.click(radio(FIRST, "The old plate"));
    await user.keyboard("{ArrowRight}");
    expect(radio(FIRST, "The young plate")).toBeChecked();
    expect(radio(FIRST, "The young plate")).toHaveFocus();
    await user.keyboard("{ArrowDown}{ArrowLeft}{ArrowLeft}");
    expect(radio(FIRST, "The old plate")).toBeChecked();
    expect(screen.getByText("1 of 2")).toBeVisible();
    expect(screen.queryByRole("group", { name: SECOND })).toBeNull();
  });

  it("has no arrows and no position for a single question", () => {
    const { section } = renderBlocks();
    section([
      {
        type: "Check",
        questions: [
          { index: 0, stem: FIRST, options: firstOptions, answer: null },
        ],
      },
    ]);

    expect(
      screen.getByRole("heading", { level: 2, name: "Check question" }),
    ).toBeVisible();
    expect(screen.getByRole("group", { name: FIRST })).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Previous question" }),
    ).toBeNull();
    expect(screen.queryByRole("button", { name: "Next question" })).toBeNull();
    expect(screen.queryByText(/^\d+ of \d+$/)).toBeNull();
  });

  it("opens on the first question not yet answered", () => {
    const { section } = renderBlocks();
    section([twoQuestions(answered, null)]);

    expect(screen.getByRole("group", { name: SECOND })).toBeVisible();
    expect(screen.queryByRole("group", { name: FIRST })).toBeNull();
    expect(screen.getByText("2 of 2")).toBeVisible();
    expect(previous()).not.toHaveAttribute("aria-disabled", "true");
    expect(next()).toHaveAttribute("aria-disabled", "true");
  });

  it("opens on the first question when every one is answered", () => {
    const { section } = renderBlocks();
    section([
      twoQuestions(answered, { ...answered, choice: 0, correct: true }),
    ]);

    expect(screen.getByRole("group", { name: FIRST })).toBeVisible();
    expect(screen.queryByRole("group", { name: SECOND })).toBeNull();
    expect(screen.getByText("1 of 2")).toBeVisible();
  });
});
