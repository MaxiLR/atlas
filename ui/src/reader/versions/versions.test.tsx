import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect } from "react";
import { describe, expect, it } from "vitest";

import {
  AtlasStoreProvider,
  useAtlasStore,
  type AtlasStore,
} from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { ExplainStyle } from "@/core/generated/ExplainStyle";
import { SectionView } from "@/reader/section/section-view";
import { createValve, drained, findSection, startedAt } from "@/test/fake-core";

import { VersionSwitch } from "./version-switch";

const ORIGINAL_TEXT = "Plates move toward each other at a few centimetres";

function Store({ onStore }: { onStore: (store: AtlasStore) => void }) {
  const store = useAtlasStore();
  useEffect(() => {
    onStore(store);
  }, [store, onStore]);
  return null;
}

/**
 * The version switch and the reading column of Convergent boundaries (one
 * stored version, the original) over the fake core, every step of a
 * generation let through unless the valve holds it. `requests` records what
 * the two components asked of the core.
 */
async function renderSection() {
  const valve = createValve();
  valve.open();
  const core = createFakeCore({
    scenario: "default",
    clock: createManualClock(startedAt),
    pace: valve.pace,
  });
  const { courses } = await core.call({ type: "ListCourses" });
  const courseId = courses.find((c) => c.topic === "Plate tectonics")!.id;
  const { course } = await core.call({ type: "OpenCourse", courseId });
  const sectionId = findSection(course, "Convergent boundaries").id;
  const opened = await core.call({ type: "OpenSection", courseId, sectionId });
  const originalId = opened.section.version.id;

  const requests: CoreRequest[] = [];
  const client: CoreClient = {
    call(request) {
      requests.push(request);
      return core.call(request);
    },
    subscribe: (listener) => core.subscribe(listener),
  };
  let store: AtlasStore | null = null;
  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <Store onStore={(given) => (store = given)} />
        <VersionSwitch courseId={courseId} sectionId={sectionId} />
        <SectionView course={course} sectionId={sectionId} />
      </AtlasStoreProvider>
    </CoreProvider>,
  );
  await act(async () => {
    await store!.send({ type: "OpenSection", courseId, sectionId });
  });
  const user = userEvent.setup();
  const article = screen.getByRole("article", {
    name: "Convergent boundaries",
  });

  /** Chooses an explain action and waits for the version it adds. */
  const explain = async (name: string, switchName: string) => {
    await user.click(within(article).getByRole("button", { name }));
    return screen.findByRole("button", { name: switchName });
  };

  return {
    core,
    valve,
    requests,
    user,
    article,
    ids: { courseId, sectionId },
    originalId,
    explain,
  };
}

/** No explain action can be chosen: each one is absent or disabled. */
function expectNoExplainAction(article: HTMLElement) {
  for (const name of ["Simpler", "More technical", "More examples"]) {
    const action = within(article).queryByRole("button", { name });
    if (action !== null) expect(action).toBeDisabled();
  }
}

describe("a section with one version", () => {
  it("shows no version switch, and offers the three explain actions after its content", async () => {
    const { article } = await renderSection();

    expect(article).toHaveTextContent(ORIGINAL_TEXT);
    expect(screen.queryByRole("button", { name: /^Version/ })).toBeNull();
    const row = within(article).getByRole("group", {
      name: "Explain this section differently",
    });
    expect(
      within(row)
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual(["Simpler", "More technical", "More examples"]);
    for (const button of within(row).getAllByRole("button")) {
      expect(button).toBeEnabled();
    }
  });
});

describe("the explain actions", () => {
  it.each<[string, ExplainStyle]>([
    ["Simpler", "Simpler"],
    ["More technical", "MoreTechnical"],
    ["More examples", "MoreExamples"],
  ])("%s sends ExplainDifferently in its style", async (name, style) => {
    const { requests, ids, user, article } = await renderSection();

    await user.click(within(article).getByRole("button", { name }));

    expect(requests).toContainEqual({
      type: "ExplainDifferently",
      ...ids,
      style,
    });
  });
});

describe("a section with several versions", () => {
  it("names the current version and its position, and lists every version in order", async () => {
    const { explain, user } = await renderSection();
    await explain("Simpler", "Version: Simpler, 2 of 2");
    const versions = await explain(
      "More examples",
      "Version: More examples, 3 of 3",
    );

    expect(versions).toHaveTextContent("More examples");
    expect(versions).toHaveTextContent("3 of 3");

    await user.click(versions);
    const items = await screen.findAllByRole("menuitemradio");
    expect(items.map((item) => item.textContent)).toEqual([
      "Original1",
      "Simpler2",
      "More examples3",
    ]);
    expect(items[2]).toHaveAttribute("aria-checked", "true");
    expect(items[0]).toHaveAttribute("aria-checked", "false");
  });

  it("choosing a version sends SelectVersion and the column shows that version at once", async () => {
    const { explain, user, requests, ids, originalId, article } =
      await renderSection();
    const versions = await explain(
      "More examples",
      "Version: More examples, 2 of 2",
    );
    expect(article).toHaveTextContent("Take an example first.");

    await user.click(versions);
    await user.click(
      await screen.findByRole("menuitemradio", { name: /Original/ }),
    );

    expect(requests).toContainEqual({
      type: "SelectVersion",
      ...ids,
      versionId: originalId,
    });
    expect(
      await screen.findByRole("button", { name: "Version: Original, 1 of 2" }),
    ).toBeVisible();
    expect(article).toHaveTextContent(ORIGINAL_TEXT);
    expect(article).not.toHaveTextContent("Take an example first.");
    expect(
      requests.filter((request) => request.type === "ExplainDifferently"),
    ).toHaveLength(1);
  });
});

describe("while a new version is generated", () => {
  it("streams it in the column, keeps the current version named and the switch and actions unavailable", async () => {
    const { explain, valve, user, article } = await renderSection();
    await explain("Simpler", "Version: Simpler, 2 of 2");

    valve.close();
    await user.click(
      within(article).getByRole("button", { name: "More examples" }),
    );
    // One step for its web search, one for the first words.
    for (let step = 0; step < 2; step += 1) {
      await act(async () => {
        await drained();
        valve.step();
      });
    }

    expect(
      await within(article).findByText(/Take an example first\./),
    ).toBeVisible();
    expect(within(article).getByRole("status")).toHaveTextContent(
      "Writing this section",
    );
    expect(
      screen.getByRole("button", { name: "Version: Simpler, 2 of 2" }),
    ).toBeDisabled();
    expectNoExplainAction(article);

    valve.open();
    expect(
      await screen.findByRole("button", {
        name: "Version: More examples, 3 of 3",
      }),
    ).toBeEnabled();
  });
});

describe("a stopped or failed attempt", () => {
  it("a failed attempt leaves the previous version current, with Retry in the status region", async () => {
    const { core, explain, user, requests, ids, article } =
      await renderSection();
    await explain("Simpler", "Version: Simpler, 2 of 2");
    core.world.fault = {
      type: "Fail",
      error: { type: "FailedAfterRetries", attempts: 4 },
      retries: 0,
    };

    await user.click(
      within(article).getByRole("button", { name: "More technical" }),
    );

    const status = await within(article).findByRole("status");
    expect(status).toHaveTextContent(
      "This section could not be written: the request failed after 4 attempts.",
    );
    expect(
      screen.getByRole("button", { name: "Version: Simpler, 2 of 2" }),
    ).toBeEnabled();

    core.world.fault = null;
    await user.click(within(article).getByRole("button", { name: "Retry" }));
    expect(requests).toContainEqual({ type: "RetrySection", ...ids });
    expect(
      await screen.findByRole("button", {
        name: "Version: More technical, 3 of 3",
      }),
    ).toBeVisible();
  });

  it("a stopped attempt keeps the previous version, which the switch shows again", async () => {
    const { core, explain, user, article } = await renderSection();
    const versions = await explain("Simpler", "Version: Simpler, 2 of 2");
    core.world.fault = { type: "Stop", once: true };

    await user.click(
      within(article).getByRole("button", { name: "More technical" }),
    );

    expect(
      await within(article).findByText("This section is incomplete."),
    ).toBeVisible();
    expect(
      within(article).getByRole("button", { name: "Retry" }),
    ).toBeVisible();
    expect(versions).toHaveAccessibleName("Version: Simpler, 2 of 2");

    await user.click(versions);
    await user.click(
      await screen.findByRole("menuitemradio", { name: /Simpler/ }),
    );

    await expect.poll(() => article.textContent).toContain("Put simply: ");
    expect(article).not.toHaveTextContent("In more technical terms:");
    expect(article).not.toHaveTextContent("This section is incomplete.");
    expect(
      screen.getByRole("button", { name: "Version: Simpler, 2 of 2" }),
    ).toBeVisible();
  });
});
