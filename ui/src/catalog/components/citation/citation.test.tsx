import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeAll, describe, expect, it } from "vitest";

import { AtlasStoreProvider, useAtlas } from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { Source } from "@/core/generated/Source";
import { Blocks } from "@/reader/section/blocks/blocks";
import { Sources } from "@/reader/section/sources/sources";
import { selectSection } from "@/store";

import { CitationScope } from "./citation";

beforeAll(() => {
  // Radix measures floating content; jsdom has no layout to observe.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

const SECTION = "section-1";

const usgs: Source = {
  number: 1,
  url: "https://pubs.usgs.gov/gip/dynamic/understanding.html",
  title: "Understanding plate motions",
};
const stern: Source = {
  number: 2,
  url: "https://doi.org/10.1029/2001RG000108",
  title: "Subduction zones",
};

const TEXT =
  "Plates move a few centimetres a year. Which plate sinks is decided by density. The slab pulls the plate behind it.";

/**
 * A section of one prose block that cites `usgs`, then `stern`, then `usgs`
 * again, shown as the reading column shows it: its blocks and its sources
 * in one citation scope, over a store fed by the test's events.
 */
function renderCitedSection() {
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
      <CitationScope>
        <Blocks
          blocks={display.blocks}
          citations={display.citations}
          sources={display.sources}
        />
        <Sources sources={display.sources} />
      </CitationScope>
    );
  }

  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <Shown />
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
  generate({
    type: "BlockCompleted",
    block: { index: 0, content: { type: "Prose", text: TEXT } },
  });
  const cite = (phrase: string, source: Source) => {
    const start = TEXT.indexOf(phrase);
    generate({
      type: "CitationAdded",
      citation: {
        block: 0,
        start,
        end: start + phrase.length,
        source: source.number,
      },
      source,
    });
  };
  cite("a few centimetres a year", usgs);
  cite("decided by density", stern);
  cite("pulls the plate behind it", usgs);

  return { requests };
}

const chips = () => screen.getAllByRole("button", { name: /^Source \d+: / });

describe("a citation", () => {
  it("is a numbered chip in the sentence whose number matches its entry in the sources, one number per source", () => {
    renderCitedSection();

    const shown = chips();
    expect(shown.map((chip) => chip.textContent)).toEqual(["1", "2", "1"]);
    expect(shown[0]).toHaveAccessibleName(
      "Source 1: Understanding plate motions",
    );
    expect(shown[1]).toHaveAccessibleName("Source 2: Subduction zones");
    expect(shown[2]).toHaveAccessibleName(
      "Source 1: Understanding plate motions",
    );

    const list = screen.getByRole("list", { name: "Sources" });
    const entries = within(list).getAllByRole("listitem");
    expect(entries).toHaveLength(2);
    expect(entries[0]).toHaveTextContent(/^1\s*Understanding plate motions/);
    expect(entries[1]).toHaveTextContent(/^2\s*Subduction zones/);

    // Each chip points assistive technology at its entry in the list.
    expect(shown[0]).toHaveAccessibleDescription(/Understanding plate motions/);
    expect(shown[1]).toHaveAccessibleDescription(/Subduction zones/);
    expect(shown[2]).toHaveAccessibleDescription(/Understanding plate motions/);
  });

  it("opens a card with the source's title and site on keyboard focus, which Escape closes leaving focus on the chip", async () => {
    const user = userEvent.setup();
    renderCitedSection();

    await user.tab();
    const [first] = chips();
    expect(first).toHaveFocus();
    const card = await screen.findByRole("dialog", { name: "Source 1" });
    expect(within(card).getByText("Understanding plate motions")).toBeVisible();
    expect(within(card).getByText("pubs.usgs.gov")).toBeVisible();

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(first).toHaveFocus();
  });

  it("opens the card on hover, whose control opens the source in the system browser without navigating", async () => {
    const user = userEvent.setup();
    const { requests } = renderCitedSection();

    await user.hover(chips()[1]!);
    const card = await screen.findByRole("dialog", { name: "Source 2" });
    expect(within(card).getByText("Subduction zones")).toBeVisible();
    expect(within(card).getByText("doi.org")).toBeVisible();

    const open = within(card).getByRole("link", { name: /Open in browser/ });
    // A click the component does not cancel would navigate the webview.
    expect(fireEvent.click(open)).toBe(false);
    await waitFor(() =>
      expect(requests).toContainEqual({
        type: "OpenExternal",
        url: stern.url,
      }),
    );
  });

  it("opens its source in the system browser when the chip is activated", async () => {
    const user = userEvent.setup();
    const { requests } = renderCitedSection();

    chips()[2]!.focus();
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(requests).toContainEqual({
        type: "OpenExternal",
        url: usgs.url,
      }),
    );
  });
});
