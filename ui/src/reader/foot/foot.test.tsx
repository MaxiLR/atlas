import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect } from "react";
import { describe, expect, it } from "vitest";

import {
  AtlasStoreProvider,
  failureInStore,
  useAtlas,
  useAtlasStore,
  type AtlasStore,
} from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import { createValve, drained, findSection, startedAt } from "@/test/fake-core";

import { ReaderFoot } from "./foot";

function Store({ onStore }: { onStore: (store: AtlasStore) => void }) {
  const store = useAtlasStore();
  useEffect(() => {
    onStore(store);
  }, [store, onStore]);
  return null;
}

/** The foot of the section the window shows, as the reader places it. */
function OpenSectionFoot({ courseId }: { courseId: string }) {
  const sectionId = useAtlas((state) => state.navigation.sectionId);
  return <ReaderFoot courseId={courseId} sectionId={sectionId} />;
}

/**
 * The foot of Plate tectonics over the fake core, every step let through
 * unless the valve holds it. `requests` records what the foot asked of the
 * core after the last `open`; `refuse` makes the core refuse the next request
 * of a type.
 */
async function renderFoot() {
  const valve = createValve();
  valve.open();
  const core = createFakeCore({
    scenario: "default",
    clock: createManualClock(startedAt),
    pace: valve.pace,
  });
  const refusals = new Map<CoreRequest["type"], CoreError>();
  const requests: CoreRequest[] = [];
  const client: CoreClient = {
    call(request) {
      requests.push(request);
      const refusal = refusals.get(request.type);
      if (refusal !== undefined) {
        refusals.delete(request.type);
        return Promise.reject(refusal);
      }
      return core.call(request);
    },
    subscribe: (listener) => core.subscribe(listener),
  };
  const { courses } = await core.call({ type: "ListCourses" });
  const courseId = courses.find((c) => c.topic === "Plate tectonics")!.id;
  let store: AtlasStore | null = null;
  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <Store onStore={(given) => (store = given)} />
        <OpenSectionFoot courseId={courseId} />
      </AtlasStoreProvider>
    </CoreProvider>,
  );
  const { course } = await act(() =>
    store!.send({ type: "OpenCourse", courseId }),
  );
  const sectionOf = (title: string) => findSection(course, title).id;

  /** Opens the section titled `title` and waits until the core answered. */
  const open = async (title: string) => {
    const sectionId = sectionOf(title);
    await act(async () => {
      await store!.send({ type: "OpenSection", courseId, sectionId });
    });
    requests.length = 0;
    return sectionId;
  };

  /** Opens the section titled `title` without waiting for it. */
  const start = async (title: string) => {
    const sectionId = sectionOf(title);
    await act(async () => {
      store!
        .send({ type: "OpenSection", courseId, sectionId })
        .catch(failureInStore);
      await drained();
    });
    requests.length = 0;
    return sectionId;
  };

  return {
    core,
    valve,
    requests,
    courseId,
    open,
    start,
    sectionOf,
    user: userEvent.setup(),
    refuse(type: CoreRequest["type"], error: CoreError) {
      refusals.set(type, error);
    },
  };
}

const next = () => screen.getByRole("button", { name: "Next" });
const goDeeper = () => screen.getByRole("button", { name: "Go deeper" });

describe("Next on the line", () => {
  it("names the following broad-pass section, and Next asks the core for it", async () => {
    const { open, user, requests, courseId } = await renderFoot();
    const sectionId = await open("Divergent boundaries");

    expect(
      screen.getByText("Next on the line: Convergent boundaries"),
    ).toBeVisible();
    await user.click(next());

    expect(requests[0]).toEqual({ type: "Next", courseId, sectionId });
    expect(
      await screen.findByText("Next on the line: Transform boundaries"),
    ).toBeVisible();
  });

  it("names the next sibling inside a deep dive", async () => {
    const { open } = await renderFoot();
    await open("Why the denser plate sinks");

    expect(
      screen.getByText("Next on the line: Continental collision"),
    ).toBeVisible();
  });

  it("after the last sibling, names the broad-pass section that follows the branch's root, and Next goes there", async () => {
    const { open, user, requests, courseId } = await renderFoot();
    const sectionId = await open("Continental collision");

    expect(
      screen.getByText("Next on the line: Transform boundaries"),
    ).toBeVisible();
    await user.click(next());

    expect(requests[0]).toEqual({ type: "Next", courseId, sectionId });
    expect(
      await screen.findByText("Next on the line: Mantle convection"),
    ).toBeVisible();
  });

  it("is replaced by a plain statement at the end of the course", async () => {
    const { open } = await renderFoot();
    await open("Slab pull and ridge push");

    expect(screen.getByText("The course ends here.")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Next" })).toBeNull();
    expect(screen.queryByText(/Next on the line/)).toBeNull();
  });

  it("is unavailable while the open section is generating", async () => {
    const { valve, start, user, requests } = await renderFoot();
    valve.close();
    await start("Transform boundaries");

    expect(next()).toHaveAttribute("aria-disabled", "true");
    await user.click(next());
    expect(requests).toEqual([]);

    await act(async () => {
      valve.open();
      await drained();
    });
    expect(next()).not.toHaveAttribute("aria-disabled", "true");
  });

  it("shows a refused Next with a way to try again", async () => {
    const { open, user, requests, courseId, refuse } = await renderFoot();
    const sectionId = await open("Divergent boundaries");
    refuse("Next", { type: "Store", message: "The database is locked." });

    await user.click(next());

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The next section could not be found.");
    expect(alert).toHaveTextContent("The database is locked.");
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(requests.filter((request) => request.type === "Next")).toEqual([
      { type: "Next", courseId, sectionId },
      { type: "Next", courseId, sectionId },
    ]);
    expect(
      await screen.findByText("Next on the line: Transform boundaries"),
    ).toBeVisible();
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

describe("Go deeper", () => {
  it("with no deep dives yet, asks the core for them", async () => {
    const { open, user, requests, courseId } = await renderFoot();
    const sectionId = await open("Divergent boundaries");

    expect(screen.getByText("Or branch here: no deep dives yet")).toBeVisible();
    await user.click(goDeeper());

    expect(requests[0]).toEqual({ type: "GoDeeper", courseId, sectionId });
    expect(
      await screen.findByText("Or branch here: 3 of 3 deep dives unread"),
    ).toBeVisible();
  });

  it("while the deep dives are requested, is busy and asks only once", async () => {
    const { open, valve, user, requests } = await renderFoot();
    await open("Divergent boundaries");
    valve.close();

    await user.click(goDeeper());

    expect(
      await screen.findByText("Or branch here: proposing deep dives…"),
    ).toBeVisible();
    expect(goDeeper()).toHaveAttribute("aria-busy", "true");
    expect(goDeeper()).toHaveAttribute("aria-disabled", "true");
    await user.click(goDeeper());
    expect(requests.filter((r) => r.type === "GoDeeper")).toHaveLength(1);

    await act(async () => {
      valve.open();
      await drained();
    });
    expect(
      screen.getByText("Or branch here: 3 of 3 deep dives unread"),
    ).toBeVisible();
    expect(goDeeper()).not.toHaveAttribute("aria-busy", "true");
  });

  it("with deep dives present, counts the unread ones and opens the first unread", async () => {
    const { open, user, requests, courseId, sectionOf } = await renderFoot();
    await open("Subduction, step by step");
    await open("Convergent boundaries");

    expect(
      screen.getByText("Or branch here: 2 of 3 deep dives unread"),
    ).toBeVisible();
    await user.click(goDeeper());

    expect(requests[0]).toEqual({
      type: "OpenSection",
      courseId,
      sectionId: sectionOf("Why the denser plate sinks"),
    });
    expect(requests.some((r) => r.type === "GoDeeper")).toBe(false);
  });

  it("shows a failed request with a way to try again", async () => {
    const { core, open, user, requests, courseId } = await renderFoot();
    const sectionId = await open("Divergent boundaries");
    core.world.offline = true;

    await user.click(goDeeper());

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Deep dives could not be proposed.");
    expect(alert).toHaveTextContent("The app is offline.");
    core.world.offline = false;
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(requests.filter((r) => r.type === "GoDeeper")).toEqual([
      { type: "GoDeeper", courseId, sectionId },
      { type: "GoDeeper", courseId, sectionId },
    ]);
    expect(
      await screen.findByText("Or branch here: 3 of 3 deep dives unread"),
    ).toBeVisible();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("is unavailable until the open section is complete", async () => {
    const { valve, start, user, requests } = await renderFoot();
    valve.close();
    await start("Transform boundaries");

    expect(goDeeper()).toHaveAttribute("aria-disabled", "true");
    await user.click(goDeeper());
    expect(requests).toEqual([]);

    await act(async () => {
      valve.open();
      await drained();
    });
    expect(goDeeper()).not.toHaveAttribute("aria-disabled", "true");
  });
});
