import { describe, expect, it, vi } from "vitest";

import { selectSection, selectSideChats, selectUsage } from "@/store";
import {
  createValve,
  drained,
  findSection,
  openCourse,
  startFake,
  startedAt,
} from "@/test/fake-core";

import { responseTypes } from "../client";
import type { CoreEvent } from "../generated/CoreEvent";
import type { CoreRequest } from "../generated/CoreRequest";

const MINUTE = 60_000;

/** Every request type a test below sends, by the describe block naming it. */
const exercised: CoreRequest["type"][] = [
  "GetSession",
  "SignIn",
  "CancelSignIn",
  "EnablePlanUsage",
  "SignOut",
  "OpenManageUsage",
  "OpenExternal",
  "ListModels",
  "SelectModel",
  "ListCourses",
  "CreateCourse",
  "OpenCourse",
  "DeleteCourse",
  "SubmitSelfRating",
  "AnswerPlacement",
  "RetryPlacementStep",
  "OpenSection",
  "StopGeneration",
  "RetrySection",
  "ExplainDifferently",
  "SelectVersion",
  "AnswerCheck",
  "Next",
  "GoDeeper",
  "Define",
  "LookupWikipedia",
  "ListGlossary",
  "DeleteGlossaryEntry",
  "ListSideChats",
  "StartSideChat",
  "OpenSideChat",
  "SendSideChatMessage",
  "StopSideChatReply",
  "DeleteSideChat",
  "GetUsage",
  "SetWindowFocused",
  "GetExportDocument",
  "ExportPdf",
];

const plateTectonicsTitles = [
  "Layers by composition",
  "Lithosphere and asthenosphere",
  "Divergent boundaries",
  "Convergent boundaries",
  "Transform boundaries",
  "Mantle convection",
  "Slab pull and ridge push",
];

function generationsOf(events: CoreEvent[], sectionId: string) {
  return events.flatMap((event) =>
    event.type === "Generation" &&
    event.target.type === "Section" &&
    event.target.sectionId === sectionId
      ? [event]
      : [],
  );
}

describe("the fake core answers every request", () => {
  it("has a test below for every request of the protocol", () => {
    expect([...exercised].sort()).toEqual(Object.keys(responseTypes).sort());
  });
});

describe("GetSession", () => {
  it("reports the signed-in plan login and keeps the library", async () => {
    const { store, state } = startFake("default");

    await store.send({ type: "GetSession" });

    expect(state().session.session).toEqual({
      type: "SignedIn",
      account: { email: "candidate@example.com" },
      planUsage: "On",
    });
    expect(state().navigation.screen).toBe("library");
  });
});

describe("SignIn", () => {
  it("waits for the browser, then signs in with plan usage on", async () => {
    const valve = createValve();
    const { core, store, state } = startFake("first-launch", {
      pace: valve.pace,
    });
    await store.send({ type: "GetSession" });
    expect(state().navigation.screen).toBe("signIn");

    const { session } = await store.send({ type: "SignIn" });

    expect(session).toEqual({ type: "SigningIn" });
    expect(state().session.session).toEqual({ type: "SigningIn" });
    expect(core.opened).toHaveLength(1);
    valve.open();
    await vi.waitFor(() =>
      expect(state().session.session).toEqual({
        type: "SignedIn",
        account: { email: "candidate@example.com" },
        planUsage: "On",
      }),
    );
    expect(state().navigation.screen).toBe("library");
  });

  it("starts the usage meter: awaiting a reading, then the reading", async () => {
    const { store, state, events } = startFake("first-launch");

    await store.send({ type: "SignIn" });
    await vi.waitFor(() =>
      expect(selectUsage(state())?.type).toBe("Connected"),
    );

    const usage = events.flatMap((event) =>
      event.type === "UsageChanged" ? [event.usage] : [],
    );
    expect(usage.map((reported) => reported.type)).toEqual([
      "AwaitingReading",
      "Connected",
    ]);
    const reading = selectUsage(state());
    expect(reading).toMatchObject({
      type: "Connected",
      reading: { stale: null },
    });
    expect(
      reading?.type === "Connected" &&
        reading.reading.windows.slice(0, 2).map((window) => window.name),
    ).toEqual(["5h", "7d"]);
  });
});

describe("CancelSignIn", () => {
  it("abandons the sign-in, which stays abandoned when the browser returns", async () => {
    const valve = createValve();
    const { store, state } = startFake("first-launch", { pace: valve.pace });
    await store.send({ type: "SignIn" });

    await store.send({ type: "CancelSignIn" });
    valve.open();
    await drained();

    expect(state().session.session).toEqual({
      type: "SignedOut",
      reason: null,
    });
  });
});

describe("EnablePlanUsage", () => {
  it("turns plan usage on and lets a course start", async () => {
    const { store, state } = startFake("plan-usage-off");

    await store.send({ type: "EnablePlanUsage" });

    expect(state().session.session).toMatchObject({ planUsage: "On" });
    await expect(
      store.send({ type: "CreateCourse", topic: "Volcanoes" }),
    ).resolves.toMatchObject({ type: "Course" });
  });
});

describe("SignOut", () => {
  it("ends the plan login and the usage meter with it, keeping the courses", async () => {
    const { store, state, events } = startFake("usage-reading");
    await store.send({ type: "GetUsage" });
    expect(selectUsage(state())?.type).toBe("Connected");

    await store.send({ type: "SignOut" });

    expect(state().session.session).toEqual({
      type: "SignedOut",
      reason: { type: "SignedOut" },
    });
    expect(state().navigation.screen).toBe("signIn");
    expect(events).toContainEqual({
      type: "UsageChanged",
      usage: { type: "NoPlanLogin" },
    });
    expect(selectUsage(state())).toEqual({ type: "NoPlanLogin" });
    const { courses } = await store.send({ type: "ListCourses" });
    expect(courses.map((course) => course.topic)).toContain("Plate tectonics");
  });
});

describe("OpenManageUsage", () => {
  it("opens ChatGPT's usage settings in the browser", async () => {
    const { core, store } = startFake();

    await expect(store.send({ type: "OpenManageUsage" })).resolves.toEqual({
      type: "Done",
    });

    expect(core.opened).toEqual(["https://chatgpt.com/settings/usage"]);
  });
});

describe("OpenExternal", () => {
  it("opens web links and refuses any other scheme", async () => {
    const { core, store } = startFake();

    await store.send({
      type: "OpenExternal",
      url: "https://en.wikipedia.org/wiki/Asthenosphere",
    });
    for (const url of ["file:///etc/passwd", "javascript:alert(1)"]) {
      await expect(
        store.send({ type: "OpenExternal", url }),
      ).rejects.toMatchObject({ type: "InvalidRequest" });
    }

    expect(core.opened).toEqual([
      "https://en.wikipedia.org/wiki/Asthenosphere",
    ]);
  });
});

describe("ListModels", () => {
  it("lists the plan's models with the first one in use", async () => {
    const { store, state } = startFake();

    await store.send({ type: "ListModels" });

    const list = state().models.list;
    expect(list?.models.length).toBeGreaterThanOrEqual(2);
    expect(list?.current).toBe(list?.models[0]?.slug);
  });
});

describe("SelectModel", () => {
  it("uses a listed model from then on and refuses one not listed", async () => {
    const { store, state } = startFake();
    const { list } = await store.send({ type: "ListModels" });
    const second = list.models[1]!.slug;

    await store.send({ type: "SelectModel", slug: second });

    expect(state().models.list?.current).toBe(second);
    await expect(
      store.send({ type: "SelectModel", slug: "not-a-model" }),
    ).rejects.toMatchObject({ type: "NotFound" });
    expect(state().models.list?.current).toBe(second);
  });
});

describe("ListCourses", () => {
  it("lists the courses most recently opened first, with their progress", async () => {
    const { store, state } = startFake();

    await store.send({ type: "ListCourses" });

    const courses = state().library.courses ?? [];
    expect(courses.map((course) => course.topic)).toEqual([
      "Plate tectonics",
      "The French Revolution",
    ]);
    expect(courses[0]).toMatchObject({
      stage: "Ready",
      progress: { read: 4, total: 10 },
      resumeTitle: "Convergent boundaries",
    });
    expect(courses[1]?.stage).toBe("Placement");
  });
});

describe("CreateCourse", () => {
  it("starts a course in placement on the topic as typed, listed first", async () => {
    const { store, state } = startFake();
    await store.send({ type: "ListCourses" });

    const { course } = await store.send({
      type: "CreateCourse",
      topic: "  Volcanoes ",
    });

    expect(course).toMatchObject({
      topic: "Volcanoes",
      stage: "Placement",
      chapters: [],
      placement: {
        step: { type: "ChooseRating" },
        minQuestions: 4,
        maxQuestions: 6,
        error: null,
      },
    });
    expect(state().navigation).toMatchObject({
      screen: "placement",
      courseId: course.id,
    });
    expect(state().library.courses?.[0]?.topic).toBe("Volcanoes");
  });

  it("refuses an empty topic", async () => {
    const { store, state } = startFake();

    await expect(
      store.send({ type: "CreateCourse", topic: "   " }),
    ).rejects.toMatchObject({ type: "InvalidRequest" });
    expect(state().library.error?.type).toBe("InvalidRequest");
  });
});

describe("OpenCourse", () => {
  it("opens a ready course on its index, resuming at the section last read", async () => {
    const { store, state } = startFake();

    const course = await openCourse(store, "Plate tectonics");

    const held = state().course.byId[course.id];
    expect(held?.chapters.map((chapter) => chapter.title)).toEqual([
      "The solid Earth",
      "Plates and their boundaries",
      "What moves the plates",
    ]);
    expect(
      held?.chapters.flatMap((chapter) => chapter.sections).map((s) => s.title),
    ).toEqual(plateTectonicsTitles);
    expect(held?.resumeAt).toBe(
      findSection(course, "Convergent boundaries").id,
    );
    expect(held).toMatchObject({ glossaryCount: 1, sideChatCount: 1 });
    expect(state().navigation.screen).toBe("reader");
  });

  it("opens a course left in placement at its next question", async () => {
    const { store, state } = startFake();

    const course = await openCourse(store, "The French Revolution");

    expect(state().navigation.screen).toBe("placement");
    const placement = state().placement.byCourse[course.id];
    expect(placement?.step).toMatchObject({
      type: "Question",
      question: { number: 2 },
    });
  });
});

describe("DeleteCourse", () => {
  it("removes the course for good", async () => {
    const { store, state } = startFake();
    const { courses } = await store.send({ type: "ListCourses" });
    const courseId = courses[0]!.id;

    await store.send({ type: "DeleteCourse", courseId });

    expect(state().library.courses?.map((course) => course.id)).not.toContain(
      courseId,
    );
    await expect(
      store.send({ type: "OpenCourse", courseId }),
    ).rejects.toMatchObject({ type: "NotFound" });
  });
});

describe("SubmitSelfRating", () => {
  it("prepares the questions, then shows the first of four to six", async () => {
    const { store, state, events } = startFake();
    const { course } = await store.send({
      type: "CreateCourse",
      topic: "Plate tectonics",
    });

    const { placement } = await store.send({
      type: "SubmitSelfRating",
      courseId: course.id,
      rating: "NewToThis",
      goal: "Overview",
    });

    expect(
      events.some(
        (event) =>
          event.type === "PlacementProgress" &&
          event.placement.step.type === "PreparingQuestions",
      ),
    ).toBe(true);
    expect(placement.step).toMatchObject({
      type: "Question",
      question: { number: 1 },
    });
    expect(
      placement.step.type === "Question" && placement.step.question.options,
    ).toHaveLength(4);
    expect(state().placement.byCourse[course.id]).toEqual(placement);
  });
});

describe("AnswerPlacement", () => {
  it("asks one question at a time, then builds the index and opens it", async () => {
    const { store, state, events } = startFake();
    const { course } = await store.send({
      type: "CreateCourse",
      topic: "Plate tectonics",
    });
    let { placement } = await store.send({
      type: "SubmitSelfRating",
      courseId: course.id,
      rating: "KnowTheBasics",
      goal: "WorkingKnowledge",
    });
    const asked: number[] = [];
    const ids = new Set<string>();
    while (placement.step.type === "Question") {
      asked.push(placement.step.question.number);
      ids.add(placement.step.question.id);
      ({ placement } = await store.send({
        type: "AnswerPlacement",
        courseId: course.id,
        questionId: placement.step.question.id,
        answer:
          asked.length === 2
            ? { type: "NotSure" }
            : { type: "Option", index: 1 },
      }));
    }

    expect(asked).toEqual([1, 2, 3, 4]);
    expect(ids.size).toBe(4);
    expect(placement.step).toEqual({ type: "Done" });
    expect(
      events.some(
        (event) =>
          event.type === "PlacementProgress" &&
          event.placement.step.type === "BuildingIndex",
      ),
    ).toBe(true);
    const held = state().course.byId[course.id];
    expect(held?.stage).toBe("Ready");
    expect(
      held?.chapters.flatMap((chapter) => chapter.sections).map((s) => s.title),
    ).toEqual(plateTectonicsTitles);
    expect(state().navigation.screen).toBe("reader");
  });

  it("refuses an answer to a question not shown", async () => {
    const { store } = startFake();
    const { course } = await store.send({
      type: "CreateCourse",
      topic: "Plate tectonics",
    });
    await store.send({
      type: "SubmitSelfRating",
      courseId: course.id,
      rating: "Expert",
      goal: "Expert",
    });

    await expect(
      store.send({
        type: "AnswerPlacement",
        courseId: course.id,
        questionId: "00000000-0000-0000-0000-000000000000",
        answer: { type: "NotSure" },
      }),
    ).rejects.toMatchObject({ type: "InvalidRequest" });
  });
});

describe("RetryPlacementStep", () => {
  it("repeats the step that failed", async () => {
    const { core, store, state } = startFake("offline");
    const { course } = await store.send({
      type: "CreateCourse",
      topic: "Volcanoes",
    });

    const failed = await store.send({
      type: "SubmitSelfRating",
      courseId: course.id,
      rating: "NewToThis",
      goal: "Overview",
    });
    expect(failed.placement).toMatchObject({
      step: { type: "PreparingQuestions" },
      error: { type: "Offline" },
    });
    expect(state().placement.byCourse[course.id]?.error).toEqual({
      type: "Offline",
    });

    core.world.offline = false;
    const { placement } = await store.send({
      type: "RetryPlacementStep",
      courseId: course.id,
    });

    expect(placement).toMatchObject({
      step: { type: "Question", question: { number: 1 } },
      error: null,
    });
    expect(state().placement.byCourse[course.id]?.error).toBeNull();
  });
});

describe("OpenSection", () => {
  it("returns a stored section with blocks of every kind, citations and sources, without generating", async () => {
    const { store, state, events } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Convergent boundaries");

    await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });

    const shown = selectSection(state(), section.id);
    expect(shown?.status).toBe("stored");
    expect(
      new Set(
        shown?.blocks.map((block) =>
          block.state === "complete" ? block.content.type : block.kind,
        ),
      ),
    ).toEqual(
      new Set(["Prose", "Visual", "Figure", "Mermaid", "Check", "Fallback"]),
    );
    expect(shown?.sources.map((source) => source.number)).toEqual([1, 2]);
    expect(shown?.citations.length).toBeGreaterThanOrEqual(2);
    expect(state().navigation.sectionId).toBe(section.id);
    expect(generationsOf(events, section.id)).toEqual([]);
  });

  it("generates an unread section, stores it and marks it read", async () => {
    const { store, state, events } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Transform boundaries");

    const { section: view } = await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });

    expect(generationsOf(events, section.id)[0]?.event).toEqual({
      type: "Started",
    });
    const shown = selectSection(state(), section.id);
    expect(shown?.status).toBe("stored");
    expect(shown?.blocks[0]).toMatchObject({
      state: "complete",
      content: { type: "Prose" },
    });
    expect(view.versions).toHaveLength(1);
    const held = state().course.byId[course.id]!;
    expect(findSection(held, "Transform boundaries")).toMatchObject({
      read: true,
      mark: "Read",
    });
    expect(state().library.courses?.[0]?.progress).toEqual({
      read: 5,
      total: 10,
    });

    events.length = 0;
    await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });
    expect(generationsOf(events, section.id)).toEqual([]);
  });

  it("resumes the course at a section only once it is read, not when its generation stops", async () => {
    const { store, state } = startFake("stopped-generation");
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Transform boundaries");

    await expect(
      store.send({
        type: "OpenSection",
        courseId: course.id,
        sectionId: section.id,
      }),
    ).rejects.toBeDefined();

    const { courses } = await store.send({ type: "ListCourses" });
    expect(courses[0]?.resumeTitle).toBe("Convergent boundaries");
    const { course: reopened } = await store.send({
      type: "OpenCourse",
      courseId: course.id,
    });
    expect(reopened.resumeAt).toBe(
      findSection(course, "Convergent boundaries").id,
    );

    await store.send({
      type: "RetrySection",
      courseId: course.id,
      sectionId: section.id,
    });
    const { course: read } = await store.send({
      type: "OpenCourse",
      courseId: course.id,
    });
    expect(read.resumeAt).toBe(section.id);
    expect(state().library.courses?.[0]?.resumeTitle).toBe(
      "Transform boundaries",
    );
  });
});

describe("StopGeneration", () => {
  it("ends the running generation with outcome stopped, storing nothing", async () => {
    const valve = createValve();
    const { store, state } = startFake("default", { pace: valve.pace });
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Transform boundaries");
    const opening = store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });
    valve.step();
    await vi.waitFor(() =>
      expect(selectSection(state(), section.id)?.blocks[0]).toMatchObject({
        state: "open",
        text: expect.stringMatching(/\S/),
      }),
    );

    await expect(
      store.send({
        type: "StopGeneration",
        courseId: course.id,
        sectionId: section.id,
      }),
    ).resolves.toEqual({ type: "Done" });
    valve.open();

    await expect(opening).rejects.toBeDefined();
    const shown = selectSection(state(), section.id);
    expect(shown?.outcome).toEqual({ type: "Stopped" });
    expect(shown?.status).toBe("incomplete");
    expect(shown?.versions).toEqual([]);
    expect(
      findSection(state().course.byId[course.id]!, "Transform boundaries").read,
    ).toBe(false);
  });
});

describe("RetrySection", () => {
  it("generates a stopped section again and stores it", async () => {
    const { store, state } = startFake("stopped-generation");
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Transform boundaries");
    await expect(
      store.send({
        type: "OpenSection",
        courseId: course.id,
        sectionId: section.id,
      }),
    ).rejects.toBeDefined();
    expect(selectSection(state(), section.id)?.outcome).toEqual({
      type: "Stopped",
    });

    await store.send({
      type: "RetrySection",
      courseId: course.id,
      sectionId: section.id,
    });

    const shown = selectSection(state(), section.id);
    expect(shown?.status).toBe("stored");
    expect(shown?.versions).toHaveLength(1);
  });
});

describe("ExplainDifferently", () => {
  it("adds a version in the asked style and makes it current", async () => {
    const { store, state, events } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Convergent boundaries");
    await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });

    const { section: view } = await store.send({
      type: "ExplainDifferently",
      courseId: course.id,
      sectionId: section.id,
      style: "Simpler",
    });

    expect(view.version.style).toBe("Simpler");
    expect(view.versions.map((version) => version.style)).toEqual([
      "Original",
      "Simpler",
    ]);
    expect(selectSection(state(), section.id)?.versionId).toBe(view.version.id);
    expect(
      generationsOf(events, section.id).every(
        (event) =>
          event.target.type === "Section" && event.target.style === "Simpler",
      ),
    ).toBe(true);
  });
});

describe("SelectVersion", () => {
  it("makes an earlier version current again", async () => {
    const { store, state } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Convergent boundaries");
    const { section: original } = await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });
    await store.send({
      type: "ExplainDifferently",
      courseId: course.id,
      sectionId: section.id,
      style: "MoreExamples",
    });

    await store.send({
      type: "SelectVersion",
      courseId: course.id,
      sectionId: section.id,
      versionId: original.version.id,
    });

    expect(selectSection(state(), section.id)?.versionId).toBe(
      original.version.id,
    );
    expect(selectSection(state(), section.id)?.versions).toHaveLength(2);
  });
});

describe("AnswerCheck", () => {
  it("says whether the answer was right and why, and keeps it on the version", async () => {
    const { store, state } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Convergent boundaries");
    const { section: view } = await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });
    const check = view.version.blocks.find(
      (block) => block.content.type === "Check",
    )!;

    const { result } = await store.send({
      type: "AnswerCheck",
      versionId: view.version.id,
      block: check.index,
      question: 0,
      choice: 1,
    });

    expect(result).toMatchObject({ choice: 1, correct: false, key: 0 });
    expect(result.explanation).not.toBe("");
    const shown = selectSection(state(), section.id)?.blocks.find(
      (block) => block.state === "complete" && block.index === check.index,
    );
    expect(
      shown?.state === "complete" &&
        shown.content.type === "Check" &&
        shown.content.questions[0]?.answer,
    ).toEqual(result);
  });
});

describe("Next", () => {
  async function nextFrom(title: string) {
    const fake = startFake();
    const course = await openCourse(fake.store, "Plate tectonics");
    const response = await fake.store.send({
      type: "Next",
      courseId: course.id,
      sectionId: findSection(course, title).id,
    });
    return { ...fake, course, response };
  }

  it("leads to the following section of the broad pass, which the store opens", async () => {
    const { state, course, response } = await nextFrom("Layers by composition");

    const following = findSection(course, "Lithosphere and asthenosphere");
    expect(response).toEqual({
      type: "NextSection",
      courseId: course.id,
      sectionId: following.id,
    });
    expect(state().navigation.sectionId).toBe(following.id);
    await vi.waitFor(() =>
      expect(selectSection(state(), following.id)?.status).toBe("stored"),
    );
  });

  it("passes over a section's deep dives to the next section of the broad pass", async () => {
    const { course, response } = await nextFrom("Convergent boundaries");

    expect(response).toMatchObject({
      sectionId: findSection(course, "Transform boundaries").id,
    });
  });

  it("goes to the next sibling inside a deep dive", async () => {
    const { course, response } = await nextFrom("Subduction, step by step");

    expect(response).toMatchObject({
      sectionId: findSection(course, "Why the denser plate sinks").id,
    });
  });

  it("returns from the last deep dive to the section after its root", async () => {
    const { course, response } = await nextFrom("Continental collision");

    expect(response).toMatchObject({
      sectionId: findSection(course, "Transform boundaries").id,
    });
  });

  it("ends the course after the last section", async () => {
    const { state, course, response } = await nextFrom(
      "Slab pull and ridge push",
    );

    expect(response).toEqual({ type: "EndOfCourse", courseId: course.id });
    expect(state().navigation.endOfCourse).toBe(true);
  });
});

describe("GoDeeper", () => {
  it("proposes deep dives under the section, shown in the index beneath it", async () => {
    const { store, state } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Transform boundaries");

    const { deepDives } = await store.send({
      type: "GoDeeper",
      courseId: course.id,
      sectionId: section.id,
    });

    expect(deepDives.length).toBeGreaterThanOrEqual(3);
    expect(deepDives.length).toBeLessThanOrEqual(6);
    for (const dive of deepDives) {
      expect(dive).toMatchObject({ kind: "DeepDive", read: false });
    }
    const held = findSection(
      state().course.byId[course.id]!,
      "Transform boundaries",
    );
    expect(held.mark).toBe("Branch");
    expect(held.deepDives.map((dive) => dive.id)).toEqual(
      deepDives.map((dive) => dive.id),
    );

    const again = await store.send({
      type: "GoDeeper",
      courseId: course.id,
      sectionId: section.id,
    });
    expect(again.deepDives.map((dive) => dive.id)).toEqual(
      deepDives.map((dive) => dive.id),
    );
  });
});

describe("Define", () => {
  it("streams a definition that joins the course glossary", async () => {
    const { store, state, events } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Convergent boundaries");
    const { section: view } = await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });
    const anchor = {
      versionId: view.version.id,
      block: 0,
      quote: "subduction",
    };

    const { entry } = await store.send({
      type: "Define",
      anchor,
      sentence: "One plate bends and slides beneath the other.",
    });

    expect(entry).toMatchObject({
      courseId: course.id,
      sectionId: section.id,
      sectionTitle: "Convergent boundaries",
      term: "subduction",
      anchor,
    });
    expect(entry.definition).not.toBe("");
    expect(
      events.some(
        (event) =>
          event.type === "Generation" && event.target.type === "Define",
      ),
    ).toBe(true);
    expect(
      state().glossary.byCourse[course.id]?.map((known) => known.term),
    ).toContain("subduction");
    expect(state().glossary.drafts).toEqual({});
    expect(state().course.byId[course.id]?.glossaryCount).toBe(2);
  });
});

describe("LookupWikipedia", () => {
  it("finds the article for a term, and says so when there is none", async () => {
    const { store, state } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const { entries } = await store.send({
      type: "ListGlossary",
      courseId: course.id,
    });
    const asthenosphere = entries.find(
      (entry) => entry.term === "asthenosphere",
    )!;

    const { result } = await store.send({
      type: "LookupWikipedia",
      entryId: asthenosphere.id,
    });

    expect(result).toMatchObject({
      type: "Found",
      title: "Asthenosphere",
      url: "https://en.wikipedia.org/wiki/Asthenosphere",
    });
    expect(
      state().glossary.byCourse[course.id]?.find(
        (entry) => entry.id === asthenosphere.id,
      )?.wikipedia,
    ).toEqual(result);

    const { entry } = await store.send({
      type: "Define",
      anchor: { ...asthenosphere.anchor, quote: "a few centimetres a year" },
      sentence: "Plates move toward each other at a few centimetres a year.",
    });
    const none = await store.send({
      type: "LookupWikipedia",
      entryId: entry.id,
    });
    expect(none.result).toEqual({ type: "NoArticle" });
  });
});

describe("ListGlossary", () => {
  it("lists the course's definitions with the section each came from", async () => {
    const { store, state } = startFake();
    const course = await openCourse(store, "Plate tectonics");

    await store.send({ type: "ListGlossary", courseId: course.id });

    expect(state().glossary.byCourse[course.id]).toEqual([
      expect.objectContaining({
        term: "asthenosphere",
        sectionTitle: "Convergent boundaries",
      }),
    ]);
  });
});

describe("DeleteGlossaryEntry", () => {
  it("removes the entry for good: the glossary, its count and a Wikipedia lookup no longer find it", async () => {
    const { store, state } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const { entries } = await store.send({
      type: "ListGlossary",
      courseId: course.id,
    });
    const entryId = entries[0]!.id;

    await expect(
      store.send({ type: "DeleteGlossaryEntry", entryId }),
    ).resolves.toEqual({ type: "Done" });

    expect(state().glossary.byCourse[course.id]).toEqual([]);
    expect(state().course.byId[course.id]?.glossaryCount).toBe(0);
    await expect(
      store.send({ type: "LookupWikipedia", entryId }),
    ).rejects.toMatchObject({ type: "NotFound" });
    const listed = await store.send({
      type: "ListGlossary",
      courseId: course.id,
    });
    expect(listed.entries).toEqual([]);
    const reopened = await store.send({
      type: "OpenCourse",
      courseId: course.id,
    });
    expect(reopened.course.glossaryCount).toBe(0);
  });

  it("does not find an entry it does not hold", async () => {
    const { store } = startFake();
    await openCourse(store, "Plate tectonics");

    await expect(
      store.send({ type: "DeleteGlossaryEntry", entryId: "no-such-entry" }),
    ).rejects.toMatchObject({ type: "NotFound" });
  });
});

describe("ListSideChats", () => {
  it("lists the course's side chats with their passage", async () => {
    const { store, state } = startFake();
    const course = await openCourse(store, "Plate tectonics");

    await store.send({ type: "ListSideChats", courseId: course.id });

    expect(selectSideChats(state(), course.id)).toEqual([
      expect.objectContaining({
        opening:
          "Its weight drags the rest of the plate behind it, a force called slab pull",
        sectionTitle: "Convergent boundaries",
      }),
    ]);
  });
});

describe("StartSideChat", () => {
  it("opens a new chat without a selection in the panel, listed first", async () => {
    const { store, state } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    await store.send({ type: "ListSideChats", courseId: course.id });

    const { chat } = await store.send({
      type: "StartSideChat",
      courseId: course.id,
      anchor: null,
    });

    expect(chat).toMatchObject({
      courseId: course.id,
      anchor: null,
      messages: [],
    });
    expect(state().navigation.panel).toEqual({
      open: true,
      tab: "sideChats",
      chatId: chat.id,
    });
    expect(selectSideChats(state(), course.id)?.[0]?.id).toBe(chat.id);
    expect(state().course.byId[course.id]?.sideChatCount).toBe(2);
  });
});

describe("OpenSideChat", () => {
  it("opens a chat with its thread and follow-up prompts", async () => {
    const { store, state } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const { chats } = await store.send({
      type: "ListSideChats",
      courseId: course.id,
    });

    await store.send({ type: "OpenSideChat", chatId: chats[0]!.id });

    const open = state().sideChats.chats[chats[0]!.id];
    expect(
      open?.thread.map((item) =>
        item.type === "Message" ? item.message.role : "Sent",
      ),
    ).toEqual(["Candidate", "Agent"]);
    expect(open?.followUps).toHaveLength(3);
  });
});

describe("SendSideChatMessage", () => {
  it("streams the reply, then offers follow-up prompts", async () => {
    const { store, state, events } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    const { chat } = await store.send({
      type: "StartSideChat",
      courseId: course.id,
      anchor: null,
    });

    const { message } = await store.send({
      type: "SendSideChatMessage",
      chatId: chat.id,
      text: "Why do plates move at all?",
    });

    expect(message).toMatchObject({ role: "Agent", state: "Complete" });
    expect(message.text).not.toBe("");
    expect(
      events.some(
        (event) =>
          event.type === "Generation" &&
          event.target.type === "SideChat" &&
          event.target.chatId === chat.id,
      ),
    ).toBe(true);
    const open = state().sideChats.chats[chat.id];
    expect(open?.thread).toEqual([
      { type: "Sent", text: "Why do plates move at all?" },
      { type: "Message", message },
    ]);
    expect(open?.reply).toBeNull();
    expect(open?.followUps.length).toBeGreaterThanOrEqual(1);
    expect(open?.followUps.length).toBeLessThanOrEqual(3);
  });
});

describe("StopSideChatReply", () => {
  it("stops the reply, keeping what arrived as a stopped message", async () => {
    const valve = createValve();
    const { store, state } = startFake("default", { pace: valve.pace });
    const course = await openCourse(store, "Plate tectonics");
    const { chat } = await store.send({
      type: "StartSideChat",
      courseId: course.id,
      anchor: null,
    });
    const replying = store.send({
      type: "SendSideChatMessage",
      chatId: chat.id,
      text: "Why do plates move at all?",
    });
    valve.step();
    valve.step();
    await vi.waitFor(() =>
      expect(state().sideChats.chats[chat.id]?.reply).not.toBeNull(),
    );

    await store.send({ type: "StopSideChatReply", chatId: chat.id });
    valve.open();

    const { message } = await replying;
    expect(message.state).toBe("Stopped");
    expect(state().sideChats.chats[chat.id]?.reply).toBeNull();
  });
});

describe("DeleteSideChat", () => {
  it("stops the reply streaming in it before it answers, then removes it for good: it lists, opens and counts no more", async () => {
    const valve = createValve();
    const { store, state, events } = startFake("default", {
      pace: valve.pace,
    });
    const course = await openCourse(store, "Plate tectonics");
    const listed = await store.send({
      type: "ListSideChats",
      courseId: course.id,
    });
    const sample = listed.chats[0]!.id;
    const { chat } = await store.send({
      type: "StartSideChat",
      courseId: course.id,
      anchor: null,
    });
    const replying = store.send({
      type: "SendSideChatMessage",
      chatId: chat.id,
      text: "Why do plates move at all?",
    });
    replying.catch(() => {});
    valve.step();
    valve.step();
    await vi.waitFor(() =>
      expect(state().sideChats.chats[chat.id]?.reply).not.toBeNull(),
    );
    const from = events.length;

    const deleting = store.send({ type: "DeleteSideChat", chatId: chat.id });
    valve.open();
    await expect(deleting).resolves.toEqual({ type: "Done" });

    const after = events.slice(from);
    expect(
      after.flatMap((event) =>
        event.type === "Generation" &&
        event.target.type === "SideChat" &&
        event.target.chatId === chat.id &&
        event.event.type === "Finished"
          ? [event.event.outcome.type]
          : [],
      ),
    ).toEqual(["Stopped"]);
    expect(
      after.filter(
        (event) =>
          (event.type === "SideChatsChanged" && event.chat.id === chat.id) ||
          (event.type === "FollowUpPrompts" && event.chatId === chat.id),
      ),
    ).toEqual([]);
    await expect(replying).rejects.toMatchObject({ type: "NotFound" });
    expect(selectSideChats(state(), course.id)?.map(({ id }) => id)).toEqual([
      sample,
    ]);
    expect(state().sideChats.chats[chat.id]).toBeUndefined();
    expect(state().navigation.panel.chatId).toBeNull();
    expect(state().course.byId[course.id]?.sideChatCount).toBe(1);

    await expect(
      store.send({ type: "OpenSideChat", chatId: chat.id }),
    ).rejects.toMatchObject({ type: "NotFound" });
    const { chats } = await store.send({
      type: "ListSideChats",
      courseId: course.id,
    });
    expect(chats.map(({ id }) => id)).not.toContain(chat.id);
    expect(chats).toHaveLength(1);
    const reopened = await store.send({
      type: "OpenCourse",
      courseId: course.id,
    });
    expect(reopened.course.sideChatCount).toBe(1);
  });

  it("does not find a chat it does not hold", async () => {
    const { store } = startFake();
    await openCourse(store, "Plate tectonics");

    await expect(
      store.send({ type: "DeleteSideChat", chatId: "no-such-chat" }),
    ).rejects.toMatchObject({ type: "NotFound" });
  });
});

describe("GetUsage", () => {
  it("answers that there is no plan login while signed out", async () => {
    const { store, state } = startFake("first-launch");

    const { usage } = await store.send({ type: "GetUsage" });

    expect(usage).toEqual({ type: "NoPlanLogin" });
    expect(selectUsage(state())).toEqual({ type: "NoPlanLogin" });
  });

  it("takes a reading when signed in without one, with no connection step", async () => {
    const { core, store, state } = startFake();

    await store.send({ type: "GetUsage" });

    await vi.waitFor(() =>
      expect(selectUsage(state())).toMatchObject({
        type: "Connected",
        reading: { stale: null },
      }),
    );
    const usage = selectUsage(state());
    expect(
      usage?.type === "Connected" &&
        usage.reading.windows
          .slice(0, 2)
          .map((window) => [window.name, window.used]),
    ).toEqual([
      ["5h", 0.38],
      ["7d", 0.61],
    ]);
    expect(core.opened).toEqual([]);
  });

  it("answers that the first reading failed, and reads again when asked again", async () => {
    const { core, store, state } = startFake();
    core.world.usageFailure = "Timeout";

    await store.send({ type: "GetUsage" });
    await vi.waitFor(() =>
      expect(selectUsage(state())).toEqual({
        type: "FirstReadFailed",
        reason: "Timeout",
      }),
    );

    core.world.usageFailure = null;
    await store.send({ type: "GetUsage" });

    await vi.waitFor(() =>
      expect(selectUsage(state())).toMatchObject({
        type: "Connected",
        reading: { stale: null },
      }),
    );
  });
});

describe("SetWindowFocused", () => {
  it("takes a reading every five minutes while the window is focused, none without focus", async () => {
    const { store, state, clock } = startFake("usage-reading");
    await store.send({ type: "GetUsage" });
    const takenAt = () => {
      const usage = selectUsage(state());
      return usage?.type === "Connected" ? usage.reading.takenAt : null;
    };
    const first = takenAt();

    await store.send({ type: "SetWindowFocused", focused: true });
    clock.advance(4 * MINUTE);
    await drained();
    expect(takenAt()).toBe(first);

    clock.advance(MINUTE);
    await vi.waitFor(() =>
      expect(takenAt()).toBe(
        new Date(Date.parse(startedAt) + 5 * MINUTE).toISOString(),
      ),
    );

    await store.send({ type: "SetWindowFocused", focused: false });
    const focused = takenAt();
    clock.advance(10 * MINUTE);
    await drained();
    expect(takenAt()).toBe(focused);
  });

  it("takes a reading after a generation completes", async () => {
    const { store, events } = startFake("usage-reading");
    const course = await openCourse(store, "Plate tectonics");
    const section = findSection(course, "Transform boundaries");

    await store.send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: section.id,
    });

    const finished = events.findIndex(
      (event) => event.type === "Generation" && event.event.type === "Finished",
    );
    expect(finished).toBeGreaterThan(-1);
    expect(
      events.slice(finished).some((event) => event.type === "UsageChanged"),
    ).toBe(true);
  });
});

describe("GetExportDocument", () => {
  it("describes the course as it stands, generating nothing", async () => {
    const { store, events } = startFake();
    const course = await openCourse(store, "Plate tectonics");
    events.length = 0;

    const { document } = await store.send({
      type: "GetExportDocument",
      courseId: course.id,
    });

    expect(document.title).toBe("Plate tectonics");
    expect(document.sections.map((section) => section.title)).toEqual([
      "Layers by composition",
      "Lithosphere and asthenosphere",
      "Divergent boundaries",
      "Convergent boundaries",
    ]);
    const transform = document.contents.find(
      (entry) => entry.title === "Transform boundaries",
    );
    expect(transform).toMatchObject({ level: "Section", included: false });
    expect(document.contents[0]).toMatchObject({
      level: "Chapter",
      depth: 0,
      title: "The solid Earth",
    });
    expect(document.glossary.map((entry) => entry.term)).toEqual([
      "asthenosphere",
    ]);
    expect(events.filter((event) => event.type === "Generation")).toEqual([]);
  });
});

describe("ExportPdf", () => {
  it("writes the PDF to the file chosen", async () => {
    const { store } = startFake();
    const course = await openCourse(store, "Plate tectonics");

    const { result } = await store.send({
      type: "ExportPdf",
      courseId: course.id,
    });

    expect(result).toMatchObject({
      type: "Saved",
      path: expect.stringMatching(/Plate tectonics\.pdf$/),
    });
  });
});
