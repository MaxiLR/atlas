import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AtlasStoreProvider } from "@/app/store";
import { CitationScope } from "@/catalog/components/citation/citation";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { Source } from "@/core/generated/Source";

import { Sources } from "./sources";

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

/** The sources of a section, in a store whose client records requests. */
function renderSources(sources: Source[]) {
  const requests: CoreRequest[] = [];
  const client: CoreClient = {
    call: (async (request: CoreRequest) => {
      requests.push(request);
      return { type: "Done" };
    }) as CoreClient["call"],
    subscribe: () => () => {},
  };

  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <main data-testid="column">
          <CitationScope>
            <Sources sources={sources} />
          </CitationScope>
        </main>
      </AtlasStoreProvider>
    </CoreProvider>,
  );
  return { requests, column: screen.getByTestId("column") };
}

describe("the sources of a section", () => {
  it("lists each source once under its number, with its title and site", () => {
    renderSources([usgs, stern]);

    expect(
      screen.getByRole("heading", { level: 2, name: "Sources" }),
    ).toBeVisible();
    const list = screen.getByRole("list", { name: "Sources" });
    const entries = within(list).getAllByRole("listitem");
    expect(entries).toHaveLength(2);
    expect(entries[0]).toHaveTextContent(
      /^1\s*Understanding plate motions.*pubs\.usgs\.gov/,
    );
    expect(entries[1]).toHaveTextContent(/^2\s*Subduction zones.*doi\.org/);
  });

  it("opens a source in the system browser without navigating the webview", async () => {
    const { requests } = renderSources([usgs, stern]);

    const link = screen.getByRole("link", { name: /Subduction zones/ });
    // A click the list does not cancel would navigate the webview.
    expect(fireEvent.click(link)).toBe(false);
    await waitFor(() =>
      expect(requests).toEqual([{ type: "OpenExternal", url: stern.url }]),
    );
  });

  it("shows nothing when the section has no sources", () => {
    const { column } = renderSources([]);

    expect(screen.queryByRole("heading", { name: "Sources" })).toBeNull();
    expect(screen.queryByRole("list")).toBeNull();
    expect(column).toBeEmptyDOMElement();
  });
});
