import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { App } from "@/App";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import { drained, startedAt } from "@/test/fake-core";
import { applyStyles, clearStyles } from "@/test/styles";

beforeAll(() => {
  // Radix measures floating content; jsdom has no layout to observe.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

afterEach(() => {
  clearStyles();
  vi.useRealTimers();
});

const longTopic =
  "The history and practice of cartography, from Ptolemy's Geography and the portolan charts to satellite geodesy and the projections behind modern web maps";

/**
 * The app on the fake core in `scenario`, home on the library. Every request
 * reaches the fake core and is recorded in `requests`, except ListCourses,
 * which follows `listing.answer`: `"answer"` lets the core answer, `"hold"`
 * keeps it waiting until `release()`, an error refuses it.
 */
function renderLibrary(
  scenario: string,
  answer: "answer" | "hold" | CoreError = "answer",
) {
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  const requests: CoreRequest[] = [];
  const listing = { answer };
  const held: (() => void)[] = [];
  const client: CoreClient = {
    call: ((request: CoreRequest) => {
      requests.push(request);
      if (request.type !== "ListCourses" || listing.answer === "answer") {
        return core.call(request);
      }
      if (listing.answer === "hold") {
        const { promise, resolve } = Promise.withResolvers<void>();
        held.push(resolve);
        return promise.then(() => core.call(request));
      }
      return Promise.reject(listing.answer);
    }) as CoreClient["call"],
    subscribe: (listener) => core.subscribe(listener),
  };
  render(
    <CoreProvider client={client}>
      <App />
    </CoreProvider>,
  );
  const courseId = (topic: string) =>
    core.world.courses.find((course) => course.topic === topic)!.id;
  return {
    core,
    requests,
    listing,
    courseId,
    user: userEvent.setup(),
    async release() {
      await waitFor(() => expect(held.length).toBeGreaterThan(0));
      await act(async () => {
        held.shift()!();
        await drained();
      });
    },
  };
}

/** The Courses list, once it shows. */
async function courseList() {
  const section = await screen.findByRole("region", { name: "Courses" });
  return within(section).getByRole("list");
}

/** The row of the course on `topic`. */
async function row(topic: string) {
  const list = await courseList();
  const button = within(list).getByRole("button", { name: topic });
  return button.closest("li")!;
}

describe("the library", () => {
  it("asks what to learn, with the hint, and shows only the topic field when there are no courses", async () => {
    renderLibrary("empty-library");

    const field = await screen.findByRole("textbox", { name: "Topic" });
    expect(field).toBeEnabled();
    expect(
      screen.getByRole("heading", {
        level: 1,
        name: "What do you want to learn?",
      }),
    ).toBeVisible();
    expect(field).toHaveAccessibleDescription(
      /Courses are written in English\./,
    );
    expect(screen.getByRole("button", { name: "Start course" })).toBeVisible();
    await act(drained);
    expect(screen.queryByRole("region", { name: "Courses" })).toBeNull();
    expect(screen.queryByRole("list")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("says the courses are loading until the core lists them", async () => {
    const fake = renderLibrary("default", "hold");

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("Loading courses"),
    );
    expect(screen.queryByRole("region", { name: "Courses" })).toBeNull();
    expect(screen.getByRole("textbox", { name: "Topic" })).toBeVisible();

    await fake.release();

    expect(await row("Plate tectonics")).toBeVisible();
    expect(screen.queryByText(/Loading courses/)).toBeNull();
  });

  it("says the list failed to load, and lists the courses when tried again", async () => {
    const fake = renderLibrary("default", { type: "Offline" });

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Courses could not be loaded.",
      ),
    );
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("The app is offline.");
    expect(screen.getByRole("textbox", { name: "Topic" })).toBeVisible();

    fake.listing.answer = "answer";
    await fake.user.click(screen.getByRole("button", { name: "Try again" }));

    expect(await row("Plate tectonics")).toBeVisible();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(
      fake.requests.filter((request) => request.type === "ListCourses"),
    ).toHaveLength(2);
  });

  it("lists the courses most recently opened first, each with where it resumes, its progress and when it was last opened", async () => {
    vi.useFakeTimers({
      toFake: ["Date"],
      now: new Date(2026, 10, 19, 15, 30),
    });
    const fake = renderLibrary("library");
    const opened: Record<string, Date> = {
      "Plate tectonics": new Date(2026, 10, 19, 9, 5),
      "The French Revolution": new Date(2026, 10, 18, 22, 40),
      "Bayesian inference": new Date(2026, 10, 18, 0, 10),
      "How central banks set interest rates": new Date(2026, 10, 12, 18, 0),
      "Japanese woodblock printing": new Date(2025, 2, 3, 12, 0),
    };
    for (const course of fake.core.world.courses) {
      const at = opened[course.topic];
      if (at !== undefined) course.lastOpenedAt = at.toISOString();
    }

    const list = await courseList();

    const topics = within(list)
      .getAllByRole("listitem")
      .map((item) => within(item).getAllByRole("button")[0]!)
      .map((button) => button.getAttribute("aria-label") ?? button.textContent);
    expect(topics.slice(0, 4)).toEqual([
      "Plate tectonics",
      "The French Revolution",
      "Bayesian inference",
      "How central banks set interest rates",
    ]);
    expect(topics.at(-1)).toBe("Japanese woodblock printing");
    expect(screen.getByRole("region", { name: "Courses" })).toHaveTextContent(
      /Courses\s*12/,
    );

    const plate = await row("Plate tectonics");
    expect(
      within(plate).getByRole("button", { name: "Plate tectonics" }),
    ).toHaveAccessibleDescription("Resumes at Convergent boundaries");
    expect(within(plate).getByText("4 of 10 read")).toBeVisible();
    const track = within(plate).getByRole("progressbar");
    expect(track).toHaveAttribute("aria-valuenow", "4");
    expect(track).toHaveAttribute("aria-valuemax", "10");
    expect(within(plate).getByText("Today, 09:05")).toBeVisible();

    const bayes = await row("Bayesian inference");
    expect(
      within(bayes).getByRole("button", { name: "Bayesian inference" }),
    ).toHaveAccessibleDescription("Resumes at Conjugate priors");
    expect(within(bayes).getByText("14 of 22 read")).toBeVisible();
    expect(within(bayes).getByText("Yesterday")).toBeVisible();

    const banks = await row("How central banks set interest rates");
    expect(within(banks).getByText("21 of 21 read")).toBeVisible();
    expect(within(banks).getByText("12 Nov")).toBeVisible();

    const prints = await row("Japanese woodblock printing");
    expect(within(prints).getByText("3 Mar 2025")).toBeVisible();
  });

  it("marks a course still in placement, and opening it resumes placement", async () => {
    const fake = renderLibrary("default");

    const french = await row("The French Revolution");
    const open = within(french).getByRole("button", {
      name: "The French Revolution",
    });
    expect(open).toHaveAccessibleDescription("In placement");
    expect(within(french).queryByRole("progressbar")).toBeNull();
    expect(within(french).queryByText(/ read$/)).toBeNull();

    await fake.user.click(open);

    expect(await screen.findByText("Question 2 of 4 to 6")).toBeVisible();
    expect(fake.requests).toContainEqual({
      type: "OpenCourse",
      courseId: fake.courseId("The French Revolution"),
    });
    expect(
      fake.requests.some((request) => request.type === "OpenSection"),
    ).toBe(false);
  });

  it("opens a course at the section last read", async () => {
    const fake = renderLibrary("default");

    await fake.user.click(
      within(await row("Plate tectonics")).getByRole("button", {
        name: "Plate tectonics",
      }),
    );

    expect(
      await screen.findByRole("article", { name: "Convergent boundaries" }),
    ).toBeVisible();
    const courseId = fake.courseId("Plate tectonics");
    const resumeAt = fake.core.world.courses.find(
      (course) => course.id === courseId,
    )!.resumeAt;
    expect(
      fake.requests.filter(
        (request) =>
          request.type === "OpenCourse" || request.type === "OpenSection",
      ),
    ).toEqual([
      { type: "OpenCourse", courseId },
      { type: "OpenSection", courseId, sectionId: resumeAt },
    ]);
  });

  it("truncates a long topic on one line and keeps it whole for assistive technology", async () => {
    renderLibrary("library");

    const long = await row(longTopic);
    await applyStyles("dark");

    const open = within(long).getByRole("button", { name: longTopic });
    expect(open).toHaveAttribute("title", longTopic);
    const name = within(long).getByText(longTopic);
    expect(getComputedStyle(name).overflow).toBe("hidden");
    expect(getComputedStyle(name).textOverflow).toBe("ellipsis");
    expect(getComputedStyle(name).whiteSpace).toBe("nowrap");
  });

  describe("the topic field", () => {
    it("starts a course on the topic and moves to placement", async () => {
      const fake = renderLibrary("empty-library");

      await fake.user.type(
        await screen.findByRole("textbox", { name: "Topic" }),
        "  Volcanoes ",
      );
      await fake.user.click(
        screen.getByRole("button", { name: "Start course" }),
      );

      expect(
        await screen.findByRole("group", { name: "How well do you know it?" }),
      ).toBeVisible();
      expect(fake.requests).toContainEqual({
        type: "CreateCourse",
        topic: "Volcanoes",
      });
    });

    it("starts a course with Enter", async () => {
      const fake = renderLibrary("empty-library");

      await fake.user.type(
        await screen.findByRole("textbox", { name: "Topic" }),
        "Volcanoes{Enter}",
      );

      expect(
        await screen.findByRole("group", { name: "How well do you know it?" }),
      ).toBeVisible();
      expect(fake.requests).toContainEqual({
        type: "CreateCourse",
        topic: "Volcanoes",
      });
    });

    it("cannot be submitted empty or blank", async () => {
      const fake = renderLibrary("empty-library");

      const field = await screen.findByRole("textbox", { name: "Topic" });
      const start = screen.getByRole("button", { name: "Start course" });
      expect(start).toBeDisabled();
      await fake.user.type(field, "   {Enter}");
      expect(start).toBeDisabled();
      await act(drained);

      expect(
        fake.requests.some((request) => request.type === "CreateCourse"),
      ).toBe(false);
      expect(screen.getByRole("textbox", { name: "Topic" })).toBeVisible();
    });

    it("is unavailable while plan usage is off, says why and offers to enable it", async () => {
      const fake = renderLibrary("plan-usage-off");

      const field = await screen.findByRole("textbox", { name: "Topic" });
      await waitFor(() => expect(field).toBeDisabled());
      expect(
        screen.getByRole("button", { name: "Start course" }),
      ).toBeDisabled();
      expect(
        screen.getByText(
          "Plan usage is off for this account, so no course can start.",
        ),
      ).toBeVisible();

      await fake.user.click(
        screen.getByRole("button", { name: "Enable plan usage" }),
      );

      expect(fake.requests).toContainEqual({ type: "EnablePlanUsage" });
      await waitFor(() => expect(field).toBeEnabled());
      expect(screen.queryByText(/so no course can start/)).toBeNull();
    });

    it("is unavailable when plan usage is not allowed, says why with no action", async () => {
      renderLibrary("plan-not-allowed");

      const field = await screen.findByRole("textbox", { name: "Topic" });
      await waitFor(() => expect(field).toBeDisabled());
      expect(
        screen.getByText(
          "Plan usage is not allowed for this account or workspace, so no course can start.",
        ),
      ).toBeVisible();
      expect(
        screen.queryByRole("button", { name: "Enable plan usage" }),
      ).toBeNull();
    });
  });

  it("opens Settings from the window bar", async () => {
    const fake = renderLibrary("default");

    await fake.user.click(
      await screen.findByRole("button", { name: "Settings" }),
    );

    expect(await screen.findByText("candidate@example.com")).toBeVisible();
  });

  describe("the row menu", () => {
    async function openMenu(user: UserEvent, topic: string) {
      const item = await row(topic);
      const trigger = within(item).getByRole("button", { name: "Course menu" });
      expect(trigger).toHaveAccessibleDescription(topic);
      await user.click(trigger);
      return screen.findByRole("menu");
    }

    it("offers Export PDF and Delete course, and Export PDF asks the core to export", async () => {
      const fake = renderLibrary("default");

      const menu = await openMenu(fake.user, "Plate tectonics");
      expect(
        within(menu)
          .getAllByRole("menuitem")
          .map((item) => item.textContent),
      ).toEqual(["Export PDF", "Delete course"]);
      await fake.user.click(
        within(menu).getByRole("menuitem", { name: "Export PDF" }),
      );

      expect(fake.requests).toContainEqual({
        type: "ExportPdf",
        courseId: fake.courseId("Plate tectonics"),
      });
    });

    it("says a PDF saved from the row menu in a notice naming the file", async () => {
      const fake = renderLibrary("default");

      const menu = await openMenu(fake.user, "Plate tectonics");
      await fake.user.click(
        within(menu).getByRole("menuitem", { name: "Export PDF" }),
      );

      const notice = await within(
        screen.getByRole("region", { name: /^Notifications/ }),
      ).findByRole("status");
      expect(notice).toHaveTextContent("PDF saved");
      expect(notice).toHaveTextContent("Plate tectonics.pdf");
      expect(screen.queryByRole("alert")).toBeNull();
    });

    it("says a refused export from the row menu in its own line, with no notice", async () => {
      const fake = renderLibrary("default");
      fake.core.world.exportFailure = {
        type: "Refused",
        error: { type: "Offline" },
      };

      const menu = await openMenu(fake.user, "Plate tectonics");
      await fake.user.click(
        within(menu).getByRole("menuitem", { name: "Export PDF" }),
      );

      const line = await screen.findByRole("alert");
      expect(line).toHaveTextContent(
        "Plate tectonics could not be exported. The app is offline.",
      );
      await act(drained);
      expect(screen.getAllByRole("alert")).toEqual([line]);
      expect(screen.queryByText("PDF export failed")).toBeNull();
    });

    it("asks to confirm a deletion, naming the course and saying it is permanent, with Cancel focused", async () => {
      const fake = renderLibrary("default");

      const menu = await openMenu(fake.user, "Plate tectonics");
      await fake.user.click(
        within(menu).getByRole("menuitem", { name: "Delete course" }),
      );

      const dialog = await screen.findByRole("alertdialog", {
        name: "Delete Plate tectonics?",
      });
      expect(dialog).toHaveAccessibleDescription(/permanent/);
      const confirm = within(dialog).getByRole("button", {
        name: "Delete course",
      });
      const cancel = within(dialog).getByRole("button", { name: "Cancel" });
      await waitFor(() => expect(cancel).toHaveFocus());
      expect(confirm).not.toHaveFocus();
    });

    it("keeps the course when the deletion is cancelled", async () => {
      const fake = renderLibrary("default");

      const menu = await openMenu(fake.user, "Plate tectonics");
      await fake.user.click(
        within(menu).getByRole("menuitem", { name: "Delete course" }),
      );
      const dialog = await screen.findByRole("alertdialog");
      await fake.user.click(
        within(dialog).getByRole("button", { name: "Cancel" }),
      );

      await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
      expect(await row("Plate tectonics")).toBeVisible();
      expect(
        fake.requests.some((request) => request.type === "DeleteCourse"),
      ).toBe(false);
    });

    it("deletes the course once confirmed, and it leaves the list", async () => {
      const fake = renderLibrary("default");
      const courseId = fake.courseId("Plate tectonics");

      const menu = await openMenu(fake.user, "Plate tectonics");
      await fake.user.click(
        within(menu).getByRole("menuitem", { name: "Delete course" }),
      );
      const dialog = await screen.findByRole("alertdialog");
      await fake.user.click(
        within(dialog).getByRole("button", { name: "Delete course" }),
      );

      expect(fake.requests).toContainEqual({ type: "DeleteCourse", courseId });
      const list = await courseList();
      await waitFor(() =>
        expect(
          within(list).queryByRole("button", { name: "Plate tectonics" }),
        ).toBeNull(),
      );
      expect(
        within(list).getByRole("button", { name: "The French Revolution" }),
      ).toBeVisible();
    });

    it("shows only the topic field once the last course is deleted", async () => {
      const fake = renderLibrary("default");

      for (const topic of ["Plate tectonics", "The French Revolution"]) {
        const menu = await openMenu(fake.user, topic);
        await fake.user.click(
          within(menu).getByRole("menuitem", { name: "Delete course" }),
        );
        const dialog = await screen.findByRole("alertdialog");
        await fake.user.click(
          within(dialog).getByRole("button", { name: "Delete course" }),
        );
        await waitFor(() =>
          expect(screen.queryByRole("alertdialog")).toBeNull(),
        );
      }

      await waitFor(() =>
        expect(screen.queryByRole("region", { name: "Courses" })).toBeNull(),
      );
      expect(screen.getByRole("textbox", { name: "Topic" })).toBeVisible();
    });
  });
});
