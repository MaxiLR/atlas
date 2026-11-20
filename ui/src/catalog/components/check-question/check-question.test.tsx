import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect } from "react";
import { afterEach, describe, expect, it } from "vitest";

import {
  AtlasStoreProvider,
  useAtlas,
  useAtlasStore,
  type AtlasStore,
} from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock, type FakeCore } from "@/core/fake";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { SectionVersion } from "@/core/generated/SectionVersion";
import { SectionView } from "@/reader/section/section-view";
import { selectOpenCourse } from "@/store";
import { findSection, startedAt } from "@/test/fake-core";
import {
  applyStyles,
  clearStyles,
  paintOf,
  schemes,
  tokenOf,
} from "@/test/styles";

afterEach(clearStyles);

const STEM =
  "A plate carrying young, warm ocean floor meets a plate carrying old, cold ocean floor. What happens next?";
const OPTIONS = [
  "The old plate sinks beneath the young one.",
  "The young plate sinks, because it is hotter and softer.",
  "Neither sinks; they build a mountain belt like the Himalaya.",
  "They slide past each other along a fault.",
];
const EXPLANATION =
  "Ocean floor grows colder and denser as it ages, so the old plate is the heavier one and sinks beneath the younger, more buoyant plate.";

function Store({ onStore }: { onStore: (store: AtlasStore) => void }) {
  const store = useAtlasStore();
  useEffect(() => {
    onStore(store);
  }, [store, onStore]);
  return null;
}

/** The section the window shows, as the reading column shows it. */
function OpenSection() {
  const course = useAtlas(selectOpenCourse);
  const sectionId = useAtlas((state) => state.navigation.sectionId);
  if (course === null || sectionId === null) return null;
  return <SectionView course={course} sectionId={sectionId} />;
}

/**
 * Plate tectonics over the fake core, its sections shown in the reading
 * column. `requests` records every request; `refuse` makes the core refuse
 * the next request of a type; `hold` keeps the next AnswerCheck waiting
 * until its `release`.
 */
async function renderCourse(prepare: (core: FakeCore) => void = () => {}) {
  const core = createFakeCore({
    scenario: "default",
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  prepare(core);
  const requests: CoreRequest[] = [];
  const refusals = new Map<CoreRequest["type"], CoreError>();
  let held: Promise<void> | null = null;
  const client: CoreClient = {
    async call(request) {
      requests.push(request);
      const refusal = refusals.get(request.type);
      if (refusal !== undefined) {
        refusals.delete(request.type);
        throw refusal;
      }
      if (request.type === "AnswerCheck" && held !== null) {
        const waiting = held;
        held = null;
        await waiting;
      }
      return core.call(request);
    },
    subscribe: (listener) => core.subscribe(listener),
  } as CoreClient;
  const { courses } = await core.call({ type: "ListCourses" });
  const courseId = courses.find((c) => c.topic === "Plate tectonics")!.id;
  let store: AtlasStore | null = null;
  render(
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        <Store onStore={(given) => (store = given)} />
        <OpenSection />
      </AtlasStoreProvider>
    </CoreProvider>,
  );
  const { course } = await act(() =>
    store!.send({ type: "OpenCourse", courseId }),
  );

  /** Opens the section titled `title`; returns the version it shows. */
  const open = async (title: string): Promise<SectionVersion> => {
    const sectionId = findSection(course, title).id;
    const { section } = await act(() =>
      store!.send({ type: "OpenSection", courseId, sectionId }),
    );
    return section.version;
  };

  return {
    core,
    courseId,
    course,
    requests,
    open,
    user: userEvent.setup(),
    refuse(type: CoreRequest["type"], error: CoreError) {
      refusals.set(type, error);
    },
    hold() {
      const { promise, resolve } = Promise.withResolvers<void>();
      held = promise;
      return () => act(async () => resolve());
    },
  };
}

/** The index of the version's check block, as the core stored it. */
function checkBlockOf(version: SectionVersion): number {
  const block = version.blocks.find(
    (candidate) => candidate.content.type === "Check",
  );
  if (block === undefined) throw new Error("the version has no check block");
  return block.index;
}

const group = (stem = STEM) => screen.getByRole("group", { name: stem });
const option = (text: string, stem = STEM) =>
  within(group(stem)).getByRole("radio", {
    name: new RegExp(`^${text.replace(/[.;,]/g, "\\$&")}`),
  });
const row = (text: string, stem = STEM) =>
  option(text, stem).closest("label") as HTMLElement;
const checkAnswer = () => screen.getByRole("button", { name: "Check answer" });
const checkedOptions = (stem = STEM) =>
  within(group(stem))
    .getAllByRole("radio")
    .filter((radio) => (radio as HTMLInputElement).checked)
    .map((radio) => radio.closest("label")?.textContent);
/** The live region that announces the outcome of checking. */
const announced = () =>
  screen
    .getAllByRole("status")
    .map((region) => region.textContent ?? "")
    .join(" ");

describe("a check question not yet answered", () => {
  it("shows its stem and four options as a radio group, none chosen, and Check answer unavailable", async () => {
    const { open, requests } = await renderCourse();
    await open("Convergent boundaries");

    expect(
      screen.getByRole("heading", { level: 2, name: "Check question" }),
    ).toBeVisible();
    const radios = within(group()).getAllByRole("radio");
    expect(radios).toHaveLength(4);
    OPTIONS.forEach((text) => expect(option(text)).toBeEnabled());
    expect(checkedOptions()).toEqual([]);
    expect(checkAnswer()).toBeDisabled();
    expect(screen.queryByText(EXPLANATION)).toBeNull();
    expect(requests.some((request) => request.type === "AnswerCheck")).toBe(
      false,
    );
  });

  it("makes Check answer available once an option is chosen", async () => {
    const { open, user } = await renderCourse();
    await open("Convergent boundaries");

    await user.click(option(OPTIONS[2]));

    expect(checkedOptions()).toEqual([expect.stringContaining(OPTIONS[2])]);
    expect(checkAnswer()).toBeEnabled();
  });

  it("moves between options with the arrow keys", async () => {
    const { open, user } = await renderCourse();
    await open("Convergent boundaries");

    await user.click(option(OPTIONS[0]));
    await user.keyboard("{ArrowDown}");
    expect(option(OPTIONS[1])).toHaveFocus();
    expect(checkedOptions()).toEqual([expect.stringContaining(OPTIONS[1])]);

    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowUp}");
    expect(option(OPTIONS[2])).toHaveFocus();
    expect(checkedOptions()).toEqual([expect.stringContaining(OPTIONS[2])]);
  });
});

describe("checking an answer", () => {
  it("asks the core with the version, the block, the question and the choice", async () => {
    const { open, user, requests } = await renderCourse();
    const version = await open("Convergent boundaries");

    await user.click(option(OPTIONS[1]));
    await user.click(checkAnswer());

    await waitFor(() =>
      expect(
        requests.filter((request) => request.type === "AnswerCheck"),
      ).toEqual([
        {
          type: "AnswerCheck",
          versionId: version.id,
          block: checkBlockOf(version),
          question: 0,
          choice: 1,
        },
      ]),
    );
  });

  it("while checking, says so and holds the choice and the button", async () => {
    const { open, user, hold } = await renderCourse();
    await open("Convergent boundaries");
    const release = hold();

    await user.click(option(OPTIONS[0]));
    await user.click(screen.getByRole("button", { name: "Check answer" }));

    const busy = await screen.findByRole("button", { name: /Checking/ });
    expect(busy).toBeDisabled();
    OPTIONS.forEach((text) => expect(option(text)).toBeDisabled());
    expect(checkedOptions()).toEqual([expect.stringContaining(OPTIONS[0])]);
    expect(screen.queryByText(EXPLANATION)).toBeNull();

    await release();
    expect(await screen.findByText(EXPLANATION)).toBeVisible();
  });

  it("a right answer is marked correct, with the explanation, the group locked and the outcome announced", async () => {
    const { open, user } = await renderCourse();
    await open("Convergent boundaries");

    await user.click(option(OPTIONS[0]));
    await user.click(checkAnswer());

    expect(await screen.findByText(EXPLANATION)).toBeVisible();
    expect(row(OPTIONS[0])).toHaveTextContent("Your answer, correct");
    expect(group()).not.toHaveTextContent("incorrect");
    expect(group()).not.toHaveTextContent("Correct answer");
    expect(announced()).toContain("Correct");
    expect(announced()).not.toContain("Incorrect");
    OPTIONS.forEach((text) => expect(option(text)).toBeDisabled());
    expect(checkedOptions()).toEqual([expect.stringContaining(OPTIONS[0])]);
    expect(screen.queryByRole("button", { name: "Check answer" })).toBeNull();
  });

  it("a wrong answer is marked incorrect, the right option is marked, with the explanation and the group locked", async () => {
    const { open, user } = await renderCourse();
    await open("Convergent boundaries");

    await user.click(option(OPTIONS[1]));
    await user.click(checkAnswer());

    expect(await screen.findByText(EXPLANATION)).toBeVisible();
    expect(row(OPTIONS[1])).toHaveTextContent("Your answer, incorrect");
    expect(row(OPTIONS[0])).toHaveTextContent("Correct answer");
    for (const text of [OPTIONS[2], OPTIONS[3]]) {
      expect(row(text)).not.toHaveTextContent(/correct/i);
    }
    expect(announced()).toContain("Incorrect");
    OPTIONS.forEach((text) => expect(option(text)).toBeDisabled());
    expect(checkedOptions()).toEqual([expect.stringContaining(OPTIONS[1])]);
    expect(screen.queryByRole("button", { name: "Check answer" })).toBeNull();
  });

  it("a failed check leaves the choice in place, says why and offers to try again", async () => {
    const { open, user, refuse, requests } = await renderCourse();
    await open("Convergent boundaries");
    refuse("AnswerCheck", { type: "Store", message: "The database is busy." });

    await user.click(option(OPTIONS[1]));
    await user.click(checkAnswer());

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The database is busy.");
    expect(checkedOptions()).toEqual([expect.stringContaining(OPTIONS[1])]);
    OPTIONS.forEach((text) => expect(option(text)).toBeEnabled());
    expect(screen.queryByText(EXPLANATION)).toBeNull();

    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByText(EXPLANATION)).toBeVisible();
    expect(row(OPTIONS[1])).toHaveTextContent("Your answer, incorrect");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(
      requests.filter((request) => request.type === "AnswerCheck"),
    ).toHaveLength(2);
  });
});

describe("a check question already answered", () => {
  it("reopens in its answered state without a request", async () => {
    const { core, open, requests, course, courseId } = await renderCourse();
    // Answered earlier, straight through the core.
    const sectionId = findSection(course, "Convergent boundaries").id;
    const { section } = await core.call({
      type: "OpenSection",
      courseId,
      sectionId,
    });
    await core.call({
      type: "AnswerCheck",
      versionId: section.version.id,
      block: checkBlockOf(section.version),
      question: 0,
      choice: 2,
    });

    await open("Convergent boundaries");

    expect(screen.getByText(EXPLANATION)).toBeVisible();
    expect(row(OPTIONS[2])).toHaveTextContent("Your answer, incorrect");
    expect(row(OPTIONS[0])).toHaveTextContent("Correct answer");
    expect(checkedOptions()).toEqual([expect.stringContaining(OPTIONS[2])]);
    OPTIONS.forEach((text) => expect(option(text)).toBeDisabled());
    expect(screen.queryByRole("button", { name: "Check answer" })).toBeNull();
    expect(requests.some((request) => request.type === "AnswerCheck")).toBe(
      false,
    );
  });
});

describe("two check questions", () => {
  const FIRST = "Where is crust neither made nor destroyed?";
  const SECOND = "How does a transform fault move?";

  /** Transform boundaries, written with two check questions. */
  function withTwoQuestions(core: FakeCore) {
    const section = core.world.courses
      .flatMap((course) => course.chapters)
      .flatMap((chapter) => chapter.sections)
      .find((candidate) => candidate.sample.title === "Transform boundaries")!;
    section.sample = {
      ...section.sample,
      blocks: [
        ...section.sample.blocks,
        {
          type: "Check",
          questions: [
            {
              stem: FIRST,
              options: [
                "At a transform boundary",
                "At a ridge",
                "At a trench",
                "In a rift",
              ],
              key: 0,
              explanation: "Plates slide past each other there.",
            },
            {
              stem: SECOND,
              options: ["Steadily", "In jerks", "Not at all", "Downward"],
              key: 1,
              explanation: "Stress builds until the fault slips at once.",
            },
          ],
        },
      ],
    };
  }

  it("shows each with its own options and Check answer, and answers one at a time", async () => {
    const { open, user, requests } = await renderCourse(withTwoQuestions);
    const version = await open("Transform boundaries");

    expect(
      screen.getByRole("heading", { level: 2, name: "Check questions" }),
    ).toBeVisible();
    expect(within(group(FIRST)).getAllByRole("radio")).toHaveLength(4);
    expect(within(group(SECOND)).getAllByRole("radio")).toHaveLength(4);
    const buttons = screen.getAllByRole("button", { name: "Check answer" });
    expect(buttons).toHaveLength(2);

    await user.click(option("In jerks", SECOND));
    expect(checkedOptions(FIRST)).toEqual([]);
    await user.click(
      within(group(SECOND).closest("form")!).getByRole("button", {
        name: "Check answer",
      }),
    );

    expect(
      await screen.findByText("Stress builds until the fault slips at once."),
    ).toBeVisible();
    expect(row("In jerks", SECOND)).toHaveTextContent("Your answer, correct");
    expect(option("At a ridge", FIRST)).toBeEnabled();
    expect(
      screen.queryByText("Plates slide past each other there."),
    ).toBeNull();
    expect(
      requests.filter((request) => request.type === "AnswerCheck"),
    ).toEqual([
      {
        type: "AnswerCheck",
        versionId: version.id,
        block: checkBlockOf(version),
        question: 1,
        choice: 1,
      },
    ]);
  });
});

describe.each(schemes)("in the %s theme", (scheme) => {
  it("right and wrong carry green and red on their rows, beside their words", async () => {
    const { open, user } = await renderCourse();
    await open("Convergent boundaries");
    await user.click(option(OPTIONS[3]));
    await user.click(checkAnswer());
    await screen.findByText(EXPLANATION);
    await applyStyles(scheme);

    const wrong = row(OPTIONS[3]);
    const right = row(OPTIONS[0]);
    expect(tokenOf(paintOf(wrong).border ?? "", wrong)).toBe("red");
    expect(tokenOf(paintOf(right).border ?? "", right)).toBe("green");
    expect(wrong).toHaveTextContent("Your answer, incorrect");
    expect(right).toHaveTextContent("Correct answer");
    for (const text of [OPTIONS[1], OPTIONS[2]]) {
      const plain = row(text);
      expect(tokenOf(paintOf(plain).border ?? "", plain)).toBe("edge");
    }
  });
});
