import { render, screen, waitFor } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { AtlasStoreProvider } from "@/app/store";
import { createCoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { ExportResult } from "@/core/generated/ExportResult";
import { drained, startedAt } from "@/test/fake-core";

import { CourseMenu } from "./course-menu";

/**
 * The course menu of Plate tectonics on the fake core. When `exported` is
 * given it answers `ExportPdf` in the fake's place, so a test decides when
 * and how the export ends. `requests` records what the menu asked.
 */
function renderMenu(exported?: () => Promise<ExportResult>) {
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
  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <CourseMenu courseId={course.id} />
      </AtlasStoreProvider>
    </CoreProvider>,
  );
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

/** Whatever a live status region says, joined. */
function statusText() {
  return screen
    .queryAllByRole("status")
    .map((status) => status.textContent ?? "")
    .join("");
}

describe("the course menu", () => {
  it("exports the course with Export PDF and announces the saved file by its name", async () => {
    const { user, requests, courseId } = renderMenu();

    await chooseExport(user);

    expect(exports(requests)).toEqual([{ type: "ExportPdf", courseId }]);
    await waitFor(() => expect(statusText()).toBe("Saved Plate tectonics.pdf"));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows Export PDF busy while the export runs, starts no second one, and is ready again after", async () => {
    const pending = Promise.withResolvers<ExportResult>();
    const { user, requests } = renderMenu(() => pending.promise);

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
    await waitFor(() => expect(statusText()).toBe("Saved Plates, revised.pdf"));
  });

  it("says nothing when the export is cancelled", async () => {
    const { user, requests } = renderMenu(() =>
      Promise.resolve({ type: "Cancelled" }),
    );

    await chooseExport(user);
    await drained();

    expect(exports(requests)).toHaveLength(1);
    expect(statusText()).toBe("");
    expect(screen.queryByRole("alert")).toBeNull();
    await openMenu(user);
    expect(
      await screen.findByRole("menuitem", { name: "Export PDF" }),
    ).not.toHaveAttribute("aria-disabled");
  });

  it("announces a failed export with its reason", async () => {
    const { user } = renderMenu(() =>
      Promise.resolve({ type: "Failed", reason: "The disk is full." }),
    );

    await chooseExport(user);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The PDF could not be exported.");
    expect(alert).toHaveTextContent("The disk is full.");
    expect(statusText()).toBe("");
  });

  it("announces an export the core refused with the core's reason", async () => {
    const refusal: CoreError = { type: "Offline" };
    const { user } = renderMenu(() => Promise.reject(refusal));

    await chooseExport(user);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The PDF could not be exported.");
    expect(alert).toHaveTextContent("The app is offline.");
  });

  it("clears an earlier failure when the next export is saved", async () => {
    let answer: ExportResult = { type: "Failed", reason: "The disk is full." };
    const { user } = renderMenu(() => Promise.resolve(answer));

    await chooseExport(user);
    await screen.findByRole("alert");
    answer = { type: "Saved", path: "/tmp/Plate tectonics.pdf" };
    await chooseExport(user);

    await waitFor(() => expect(statusText()).toBe("Saved Plate tectonics.pdf"));
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
