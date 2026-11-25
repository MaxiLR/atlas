import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { useEffect } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Notices } from "@/app/notices";
import {
  AtlasStoreProvider,
  useAtlasStore,
  type AtlasStore,
} from "@/app/store";
import { createCoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { ExportResult } from "@/core/generated/ExportResult";
import { drained, openCourse, startedAt } from "@/test/fake-core";
import { ThemeProvider } from "@/theme/provider";

import { CourseMenu } from "./course-menu";

/** Sonner closes a notice without a duration of its own after 4 s. */
const SONNER_DEFAULT_DURATION = 4000;

afterEach(() => {
  vi.useRealTimers();
});

function Store({ onStore }: { onStore: (store: AtlasStore) => void }) {
  const store = useAtlasStore();
  useEffect(() => {
    onStore(store);
  }, [store, onStore]);
  return null;
}

/**
 * The course menu of Plate tectonics, open in the reader on the fake core,
 * in a course bar of its own beside the notices. When `exported` is given it
 * answers `ExportPdf` in the fake's place, so a test decides when and how
 * the export ends. `requests` records what the menu asked.
 */
async function renderMenu(exported?: () => Promise<ExportResult>) {
  const core = createFakeCore({
    scenario: "default",
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  const course = core.world.courses.find(
    (each) => each.topic === "Plate tectonics",
  );
  if (course === undefined) throw new Error("no Plate tectonics");
  const requests: CoreRequest[] = [];
  const client = createCoreClient({
    async request(request) {
      requests.push(request);
      if (request.type === "ExportPdf" && exported !== undefined) {
        return { type: "Exported", result: await exported() };
      }
      return core.call(request);
    },
    subscribe: (listener) => core.subscribe(listener),
  });
  let store: AtlasStore | null = null;
  render(
    <ThemeProvider>
      <CoreProvider client={client}>
        <AtlasStoreProvider>
          <Store onStore={(given) => (store = given)} />
          <header>
            <CourseMenu courseId={course.id} />
          </header>
          <Notices />
        </AtlasStoreProvider>
      </CoreProvider>
    </ThemeProvider>,
  );
  await act(() => openCourse(store!, "Plate tectonics"));
  expect(store!.state.getState().navigation.screen).toBe("reader");
  return {
    courseId: course.id,
    requests,
    user: userEvent.setup({ pointerEventsCheck: 0 }),
  };
}

function exports(requests: CoreRequest[]) {
  return requests.filter((request) => request.type === "ExportPdf");
}

async function openMenu(user: UserEvent) {
  await user.click(screen.getByRole("button", { name: "Course menu" }));
  return screen.findByRole("menu");
}

async function chooseExport(user: UserEvent) {
  await openMenu(user);
  await user.click(await screen.findByRole("menuitem", { name: "Export PDF" }));
}

/** The list Sonner places the notices in. */
function notices() {
  return screen.getByRole("region", { name: /^Notifications/ });
}

/** The words beside the course menu: none, it is an icon button. */
function courseBarText() {
  return screen.getByRole("banner").textContent;
}

const elapse = (ms: number) => act(() => vi.advanceTimersByTime(ms));

/** Sonner's clock: the timeouts and frames it schedules, and the time. */
function fakeSonnerClock() {
  vi.useFakeTimers({
    toFake: [
      "setTimeout",
      "clearTimeout",
      "requestAnimationFrame",
      "cancelAnimationFrame",
      "Date",
    ],
  });
}

describe("the course menu", () => {
  it("exports the course with Export PDF and says the PDF was saved, naming the file, in a notice", async () => {
    const { user, requests, courseId } = await renderMenu();

    await chooseExport(user);

    expect(exports(requests)).toEqual([{ type: "ExportPdf", courseId }]);
    const notice = await within(notices()).findByRole("status");
    expect(notice).toHaveTextContent("PDF saved");
    expect(notice).toHaveTextContent("Plate tectonics.pdf");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryAllByRole("status")).toEqual([notice]);
    expect(courseBarText()).toBe("");
  });

  it("shows Export PDF busy while the export runs, starts no second one, and is ready again after", async () => {
    const pending = Promise.withResolvers<ExportResult>();
    const { user, requests } = await renderMenu(() => pending.promise);

    await chooseExport(user);
    await openMenu(user);
    const busy = await screen.findByRole("menuitem", {
      name: "Exporting PDF…",
    });
    expect(busy).toHaveAttribute("aria-disabled", "true");
    expect(screen.queryByRole("menuitem", { name: "Export PDF" })).toBeNull();
    await user.click(busy);
    expect(exports(requests)).toHaveLength(1);

    pending.resolve({
      type: "Saved",
      path: "/Users/candidate/Desktop/Plates, revised.pdf",
    });
    const ready = await screen.findByRole("menuitem", { name: "Export PDF" });
    expect(ready).not.toHaveAttribute("aria-disabled");
    await user.keyboard("{Escape}");
    const notice = await within(notices()).findByRole("status");
    expect(notice).toHaveTextContent("PDF saved");
    expect(notice).toHaveTextContent("Plates, revised.pdf");
    expect(courseBarText()).toBe("");
  });

  it("says nothing when the export is cancelled", async () => {
    const { user, requests } = await renderMenu(() =>
      Promise.resolve({ type: "Cancelled" }),
    );

    await chooseExport(user);
    await act(drained);

    expect(exports(requests)).toHaveLength(1);
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(courseBarText()).toBe("");
    await openMenu(user);
    expect(
      await screen.findByRole("menuitem", { name: "Export PDF" }),
    ).not.toHaveAttribute("aria-disabled");
  });

  it("says a failed export with its reason in a notice that stays until dismissed", async () => {
    const pending = Promise.withResolvers<ExportResult>();
    const { user } = await renderMenu(() => pending.promise);
    await chooseExport(user);

    fakeSonnerClock();
    await act(async () => {
      pending.resolve({ type: "Failed", reason: "The disk is full." });
      await pending.promise;
    });
    elapse(50);

    const notice = within(notices()).getByRole("alert");
    expect(notice).toHaveTextContent("PDF export failed");
    expect(notice).toHaveTextContent("The disk is full.");
    expect(screen.queryAllByRole("alert")).toEqual([notice]);
    expect(screen.queryByRole("status")).toBeNull();
    expect(courseBarText()).toBe("");

    elapse(SONNER_DEFAULT_DURATION * 3);
    expect(within(notices()).getByRole("alert")).toHaveTextContent(
      "PDF export failed",
    );

    vi.useRealTimers();
    await user.click(
      within(within(notices()).getByRole("alert")).getByRole("button", {
        name: "Dismiss",
      }),
    );
    await expect.poll(() => screen.queryByRole("alert")).toBeNull();
  });

  it("says an export the core refused with the core's reason in a notice that stays until dismissed", async () => {
    const pending = Promise.withResolvers<ExportResult>();
    const { user } = await renderMenu(() => pending.promise);
    await chooseExport(user);

    fakeSonnerClock();
    const refusal: CoreError = { type: "Offline" };
    await act(async () => {
      pending.reject(refusal);
      await pending.promise.catch(() => {});
    });
    elapse(50);

    const notice = within(notices()).getByRole("alert");
    expect(notice).toHaveTextContent("PDF export failed");
    expect(notice).toHaveTextContent("The app is offline.");
    expect(courseBarText()).toBe("");

    elapse(SONNER_DEFAULT_DURATION * 3);
    expect(within(notices()).getByRole("alert")).toHaveTextContent(
      "The app is offline.",
    );

    vi.useRealTimers();
    await user.click(
      within(within(notices()).getByRole("alert")).getByRole("button", {
        name: "Dismiss",
      }),
    );
    await expect.poll(() => screen.queryByRole("alert")).toBeNull();
  });

  it("replaces the failure notice when the next export is saved", async () => {
    let answer: ExportResult = { type: "Failed", reason: "The disk is full." };
    const { user } = await renderMenu(() => Promise.resolve(answer));

    await chooseExport(user);
    await within(notices()).findByRole("alert");
    answer = { type: "Saved", path: "/tmp/Plate tectonics.pdf" };
    await chooseExport(user);

    const notice = await within(notices()).findByRole("status");
    expect(notice).toHaveTextContent("PDF saved");
    expect(notice).toHaveTextContent("Plate tectonics.pdf");
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });
});
