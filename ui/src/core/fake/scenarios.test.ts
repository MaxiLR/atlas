import { describe, expect, it, vi } from "vitest";

import { selectSection, selectUsage } from "@/store";
import { findSection, openCourse, startFake } from "@/test/fake-core";

import type { CoreEvent } from "../generated/CoreEvent";
import { createFakeCore, scenarioNames } from "./index";

const named = [
  "default",
  "first-launch",
  "plan-usage-off",
  "plan-not-allowed",
  "empty-library",
  "plan-limit",
  "plan-limit-no-reset",
  "offline",
  "failed-after-retries",
  "stopped-generation",
  "search-refused",
  "usage-reading",
  "usage-stale",
  "usage-at-limit",
];

/** Opens Plate tectonics and asks for its first section never read. */
async function openUnread(scenario: string) {
  const fake = startFake(scenario);
  const course = await openCourse(fake.store, "Plate tectonics");
  const section = findSection(course, "Transform boundaries");
  const opened = await fake.store
    .send({ type: "OpenSection", courseId: course.id, sectionId: section.id })
    .then(
      (answer) => ({ answer, error: null }),
      (error: unknown) => ({ answer: null, error }),
    );
  return {
    ...fake,
    course,
    opened,
    shown: selectSection(fake.state(), section.id),
  };
}

async function usageIn(scenario: string) {
  const { store, state } = startFake(scenario);
  await store.send({ type: "GetUsage" });
  return selectUsage(state());
}

function activities(events: CoreEvent[]) {
  return events.flatMap((event) =>
    event.type === "Generation" && event.event.type === "Activity"
      ? [event.event.activity]
      : [],
  );
}

describe("the fake core's scenarios", () => {
  it("are discovered from their files, each named scenario among them", () => {
    expect(scenarioNames).toEqual(expect.arrayContaining(named));
  });

  it("has three usage scenarios, none of them a connection step", () => {
    expect(
      scenarioNames.filter((name) => name.startsWith("usage-")).sort(),
    ).toEqual(["usage-at-limit", "usage-reading", "usage-stale"]);
  });

  it("refuses a scenario it does not know", () => {
    expect(() => createFakeCore({ scenario: "no-such-scenario" })).toThrow(
      /no-such-scenario/,
    );
  });

  it("default: signed in with plan usage on, courses present", async () => {
    const { store, state } = startFake("default");
    await store.send({ type: "GetSession" });
    await store.send({ type: "ListCourses" });

    expect(state().session.session).toMatchObject({
      type: "SignedIn",
      planUsage: "On",
    });
    expect(state().library.courses?.length).toBeGreaterThanOrEqual(2);
    expect(state().navigation.screen).toBe("library");
  });

  it("first-launch: signed out with no reason, no courses, on the sign-in screen", async () => {
    const { store, state } = startFake("first-launch");
    await store.send({ type: "GetSession" });
    await store.send({ type: "ListCourses" });

    expect(state().session.session).toEqual({
      type: "SignedOut",
      reason: null,
    });
    expect(state().library.courses).toEqual([]);
    expect(state().navigation.screen).toBe("signIn");
  });

  it("plan-usage-off: signed in without the plan, so no course can start", async () => {
    const { store, state } = startFake("plan-usage-off");
    await store.send({ type: "GetSession" });

    expect(state().session.session).toMatchObject({
      type: "SignedIn",
      planUsage: "Off",
    });
    await expect(
      store.send({ type: "CreateCourse", topic: "Volcanoes" }),
    ).rejects.toEqual({ type: "PlanUsageOff" });
    expect(state().library.error).toEqual({ type: "PlanUsageOff" });
  });

  it("plan-not-allowed: signed in as not allowed, which enabling does not change", async () => {
    const { store, state } = startFake("plan-not-allowed");
    await store.send({ type: "GetSession" });
    await store.send({ type: "EnablePlanUsage" });

    expect(state().session.session).toMatchObject({
      type: "SignedIn",
      planUsage: "NotAllowed",
    });
    await expect(
      store.send({ type: "CreateCourse", topic: "Volcanoes" }),
    ).rejects.toEqual({ type: "PlanNotAllowed" });
  });

  it("empty-library: signed in with no courses", async () => {
    const { store, state } = startFake("empty-library");
    await store.send({ type: "GetSession" });
    await store.send({ type: "ListCourses" });

    expect(state().session.session).toMatchObject({ type: "SignedIn" });
    expect(state().library.courses).toEqual([]);
    expect(state().navigation.screen).toBe("library");
  });

  it("plan-limit: a generation stops at the plan's limit, with the reset time", async () => {
    const { shown, opened } = await openUnread("plan-limit");

    expect(opened.error).toMatchObject({
      type: "PlanLimit",
      resetsAt: expect.any(String),
    });
    expect(shown?.status).toBe("incomplete");
    expect(shown?.error).toMatchObject({
      type: "PlanLimit",
      resetsAt: expect.any(String),
    });
  });

  it("plan-limit-no-reset: the plan's limit without a reset time", async () => {
    const { shown } = await openUnread("plan-limit-no-reset");

    expect(shown?.status).toBe("incomplete");
    expect(shown?.error).toEqual({ type: "PlanLimit", resetsAt: null });
  });

  it("offline: stored sections open, anything needing a request says offline", async () => {
    const { store, state, course, shown } = await openUnread("offline");

    expect(shown?.error).toEqual({ type: "Offline" });
    const stored = findSection(course, "Convergent boundaries");
    await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: stored.id,
    });
    expect(selectSection(state(), stored.id)?.status).toBe("stored");
  });

  it("failed-after-retries: a generation retries three times, then fails", async () => {
    const { shown, events } = await openUnread("failed-after-retries");

    expect(
      activities(events).flatMap((activity) =>
        activity.type === "Retrying" ? [activity.attempt] : [],
      ),
    ).toEqual([2, 3, 4]);
    expect(shown?.status).toBe("incomplete");
    expect(shown?.error).toEqual({ type: "FailedAfterRetries", attempts: 4 });
  });

  it("stopped-generation: a section's generation ends stopped", async () => {
    const { shown } = await openUnread("stopped-generation");

    expect(shown).toMatchObject({
      status: "incomplete",
      outcome: { type: "Stopped" },
    });
    expect(shown?.blocks.length).toBeGreaterThan(0);
  });

  it("search-refused: sections are written without web search", async () => {
    const { opened, events } = await openUnread("search-refused");

    expect(opened.answer?.section.version).toMatchObject({
      writtenWithoutSearch: true,
      citations: [],
      sources: [],
    });
    expect(activities(events)).not.toContainEqual({ type: "Searching" });
  });

  it("search allowed: a section with sources searches and cites them", async () => {
    const { store, events } = startFake("default");
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Subduction, step by step");

    const { section: view } = await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });

    expect(view.version.writtenWithoutSearch).toBe(false);
    expect(view.version.sources.length).toBeGreaterThan(0);
    expect(activities(events)).toContainEqual({ type: "Searching" });
  });

  it("first-launch: no plan login, so the meter has nothing to show", async () => {
    expect(await usageIn("first-launch")).toEqual({ type: "NoPlanLogin" });
  });

  it("default: signed in, the meter reads usage with no connection step", async () => {
    const { store, state } = startFake("default");

    await store.send({ type: "GetUsage" });

    await vi.waitFor(() =>
      expect(selectUsage(state())).toMatchObject({
        type: "Connected",
        reading: { stale: null },
      }),
    );
  });

  it("usage-reading: the 5-hour and weekly windows, then the others, fresh", async () => {
    const usage = await usageIn("usage-reading");

    expect(usage?.type).toBe("Connected");
    const reading = usage?.type === "Connected" ? usage.reading : null;
    expect(reading?.stale).toBeNull();
    expect(reading?.windows.map((window) => window.name).slice(0, 2)).toEqual([
      "5h",
      "7d",
    ]);
    expect(reading?.windows.length).toBeGreaterThan(2);
    expect(reading?.windows[0]).toMatchObject({
      level: "Normal",
      resetsAt: expect.any(String),
    });
  });

  it("usage-stale: the last reading stays, marked stale with the reason", async () => {
    const usage = await usageIn("usage-stale");

    expect(usage).toMatchObject({
      type: "Connected",
      reading: { stale: "Network" },
    });
    expect(
      usage?.type === "Connected" && usage.reading.windows.length,
    ).toBeGreaterThanOrEqual(2);
  });

  it("usage-at-limit: a window at its limit", async () => {
    const usage = await usageIn("usage-at-limit");

    expect(
      usage?.type === "Connected" &&
        usage.reading.windows.find((window) => window.level === "Limit"),
    ).toMatchObject({ used: 1 });
  });
});
