import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "@/App";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import {
  frenchRevolutionQuestions,
  plateTectonicsQuestions,
} from "@/core/fake/sample-course";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import { createValve, drained, startedAt } from "@/test/fake-core";
import { applyStyles, clearStyles, tokenPaintOf } from "@/test/styles";

afterEach(clearStyles);

/**
 * The app on the fake core in `scenario`, home on the library. Every request
 * reaches the fake core and is recorded in `requests`; a request whose type
 * is in `hold` waits until `release()`.
 */
function renderApp(
  scenario: string,
  {
    pace = () => Promise.resolve(),
    hold = [],
  }: { pace?: () => Promise<void>; hold?: CoreRequest["type"][] } = {},
) {
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace,
  });
  const requests: CoreRequest[] = [];
  const held: (() => void)[] = [];
  const client: CoreClient = {
    call: ((request: CoreRequest) => {
      requests.push(request);
      if (!hold.includes(request.type)) return core.call(request);
      const { promise, resolve } = Promise.withResolvers<void>();
      held.push(resolve);
      return promise.then(() => core.call(request));
    }) as CoreClient["call"],
    subscribe: (listener) => core.subscribe(listener),
  };
  render(
    <CoreProvider client={client}>
      <App />
    </CoreProvider>,
  );
  return {
    core,
    requests,
    user: userEvent.setup(),
    /** The placement requests sent, in order. */
    placementRequests: () =>
      requests.filter((request) =>
        ["SubmitSelfRating", "AnswerPlacement", "RetryPlacementStep"].includes(
          request.type,
        ),
      ),
    /** The AnswerPlacement requests sent, in order. */
    answersSent: () =>
      requests.filter(
        (
          request,
        ): request is Extract<CoreRequest, { type: "AnswerPlacement" }> =>
          request.type === "AnswerPlacement",
      ),
    async release() {
      await waitFor(() => expect(held.length).toBeGreaterThan(0));
      await act(async () => {
        held.shift()!();
        await drained();
      });
    },
  };
}

async function startCourse(user: UserEvent, topic = "Plate tectonics") {
  await user.type(await screen.findByRole("textbox", { name: "Topic" }), topic);
  await user.click(screen.getByRole("button", { name: "Start course" }));
}

/** Chooses a self-rating and a goal, then continues. */
async function rate(
  user: UserEvent,
  rating = "Know the basics",
  goal = "Working knowledge",
) {
  const ratings = await screen.findByRole("group", {
    name: "How well do you know it?",
  });
  await user.click(within(ratings).getByRole("radio", { name: rating }));
  const goals = screen.getByRole("group", { name: "What is your goal?" });
  await user.click(within(goals).getByRole("radio", { name: goal }));
  await user.click(screen.getByRole("button", { name: "Continue" }));
}

/** The question group of `stem`, once it shows. */
function question(stem: string) {
  return screen.findByRole("group", { name: stem });
}

/** What each step mark in the window bar says, in order. */
function marks() {
  const list = screen.getByRole("list", { name: "Questions" });
  return within(list)
    .getAllByRole("listitem")
    .map((mark) => mark.textContent);
}

/** Nothing on the screen says or paints whether an answer was right. */
async function expectNoMarking() {
  const main = screen.getByRole("main");
  expect(main).not.toHaveTextContent(
    /\b(correct|incorrect|right answer|wrong answer|well done|not quite)\b/i,
  );
  expect(main.querySelectorAll('[aria-invalid="true"]')).toHaveLength(0);
  for (const scheme of ["light", "dark"] as const) {
    await applyStyles(scheme);
    for (const element of main.querySelectorAll("*")) {
      const tokens = Object.values(tokenPaintOf(element));
      expect(tokens).not.toContain("green");
      expect(tokens).not.toContain("red");
      expect(tokens).not.toContain("amber");
    }
  }
}

const stems = plateTectonicsQuestions.map((draft) => draft.stem);

describe("placement", () => {
  it("asks for the self-rating and the goal on one screen, and Continue needs both", async () => {
    const fake = renderApp("empty-library");
    await startCourse(fake.user);

    const ratings = await screen.findByRole("group", {
      name: "How well do you know it?",
    });
    const goals = screen.getByRole("group", { name: "What is your goal?" });
    expect(ratings).toHaveFocus();
    expect(within(ratings).getAllByRole("radio")).toHaveLength(4);
    for (const name of [
      "New to this",
      "Know the basics",
      "Use it regularly",
      "Expert",
    ]) {
      expect(within(ratings).getByRole("radio", { name })).toBeVisible();
    }
    expect(within(goals).getAllByRole("radio")).toHaveLength(3);
    for (const name of ["Overview", "Working knowledge", "Expert"]) {
      expect(within(goals).getByRole("radio", { name })).toBeVisible();
    }
    expect(
      screen.getByRole("heading", { level: 1, name: "Plate tectonics" }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Library" })).toBeVisible();

    const next = screen.getByRole("button", { name: "Continue" });
    expect(next).toBeDisabled();
    await fake.user.click(
      within(ratings).getByRole("radio", { name: "Use it regularly" }),
    );
    expect(next).toBeDisabled();
    await fake.user.click(within(goals).getByRole("radio", { name: "Expert" }));
    expect(next).toBeEnabled();
    expect(fake.placementRequests()).toEqual([]);

    await fake.user.click(next);

    await question(stems[0]!);
    expect(fake.placementRequests()).toEqual([
      {
        type: "SubmitSelfRating",
        courseId: expect.any(String),
        rating: "UseItRegularly",
        goal: "Expert",
      },
    ]);
  });

  it("says a few questions are being prepared while they are drafted, and never shows the ladder", async () => {
    const valve = createValve();
    const fake = renderApp("empty-library", { pace: valve.pace });
    await startCourse(fake.user);
    await rate(fake.user);

    const waiting = await screen.findByRole("status");
    expect(waiting).toHaveTextContent("Preparing a few questions");
    expect(screen.queryByRole("radio")).toBeNull();
    expect(screen.queryByRole("button", { name: "Continue" })).toBeNull();
    expect(document.body).not.toHaveTextContent(/ladder|rung|concept/i);

    act(() => valve.open());

    expect(await question(stems[0]!)).toBeVisible();
    expect(screen.queryByText("Preparing a few questions")).toBeNull();
    expect(document.body).not.toHaveTextContent(/ladder|rung|concept/i);
  });

  it("shows one question at a time with four options and Not sure, the counter and the step marks, and focus on the question", async () => {
    const fake = renderApp("empty-library");
    await startCourse(fake.user);
    await rate(fake.user);

    const group = await question(stems[0]!);
    expect(group).toHaveFocus();
    expect(screen.getByText("Question 1 of 4 to 6")).toBeVisible();
    expect(marks()).toEqual([
      "Question 1, current",
      "Question 2, to come",
      "Question 3, to come",
      "Question 4, to come",
      "Question 5, may not be asked",
      "Question 6, may not be asked",
    ]);
    const radios = within(group).getAllByRole("radio");
    expect(radios).toHaveLength(5);
    plateTectonicsQuestions[0]!.options.forEach((option, index) => {
      expect(radios[index]).toHaveAccessibleName(option);
      const row = radios[index]!.closest("label")!;
      expect(within(row).getByText(String(index + 1))).toBeVisible();
    });
    expect(radios[4]).toHaveAccessibleName("Not sure");
    expect(within(radios[4]!.closest("label")!).getByText("0")).toBeVisible();
    for (const radio of radios) expect(radio).not.toBeChecked();
    expect(
      screen.getByText(
        "Nothing is marked right or wrong here. Your answers only decide where the course starts.",
      ),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    expect(screen.queryByText(stems[1]!)).toBeNull();
  });

  it("chooses an option with the keys 1 to 4, and Not sure with 0", async () => {
    const fake = renderApp("empty-library");
    await startCourse(fake.user);
    await rate(fake.user);
    const group = await question(stems[0]!);
    const options = plateTectonicsQuestions[0]!.options;

    const keyed: [string, string][] = [
      ["3", options[2]!],
      ["1", options[0]!],
      ["0", "Not sure"],
      ["4", options[3]!],
      ["2", options[1]!],
    ];
    for (const [key, name] of keyed) {
      await fake.user.keyboard(key);
      const chosen = within(group).getByRole("radio", { name });
      expect(chosen).toBeChecked();
      expect(
        within(group)
          .getAllByRole("radio")
          .filter((radio) => (radio as HTMLInputElement).checked),
      ).toEqual([chosen]);
    }
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
    await fake.user.keyboard("5");
    expect(
      within(group).getByRole("radio", { name: options[1] }),
    ).toBeChecked();
    expect(fake.placementRequests()).toHaveLength(1);
  });

  it("continues with Enter, sends each answer, moves focus to each new question, then builds the index and opens the first section", async () => {
    const valve = createValve();
    valve.open();
    const fake = renderApp("empty-library", { pace: valve.pace });
    await startCourse(fake.user);
    await rate(fake.user);

    const keys = ["3", "0", "1"];
    for (const [index, key] of keys.entries()) {
      expect(await question(stems[index]!)).toHaveFocus();
      expect(screen.getByText(`Question ${index + 1} of 4 to 6`)).toBeVisible();
      await fake.user.keyboard(key);
      await fake.user.keyboard("{Enter}");
    }
    const fourth = await question(stems[3]!);
    expect(fourth).toHaveFocus();
    expect(marks()).toEqual([
      "Question 1, answered",
      "Question 2, answered",
      "Question 3, answered",
      "Question 4, current",
      "Question 5, may not be asked",
      "Question 6, may not be asked",
    ]);
    await fake.user.click(
      within(fourth).getByRole("radio", {
        name: plateTectonicsQuestions[3]!.options[3],
      }),
    );
    valve.close();
    await fake.user.click(screen.getByRole("button", { name: "Continue" }));

    expect(await screen.findByRole("status")).toHaveTextContent(
      "Building the index",
    );
    expect(screen.queryByRole("radio")).toBeNull();

    act(() => valve.open());

    expect(
      await screen.findByRole("article", { name: "Layers by composition" }),
    ).toBeVisible();
    expect(screen.getByRole("navigation", { name: "Index" })).toBeVisible();
    const answers = fake.answersSent();
    expect(answers.map((request) => request.answer)).toEqual([
      { type: "Option", index: 2 },
      { type: "NotSure" },
      { type: "Option", index: 0 },
      { type: "Option", index: 3 },
    ]);
    expect(new Set(answers.map((request) => request.questionId)).size).toBe(4);
    expect(fake.requests).toContainEqual({
      type: "OpenSection",
      courseId: answers[0]!.courseId,
      sectionId: expect.any(String),
    });
  });

  it("marks no answer right or wrong, in words or in colour, after answering", async () => {
    const fake = renderApp("empty-library", { hold: ["AnswerPlacement"] });
    await startCourse(fake.user);
    await rate(fake.user);
    const group = await question(stems[0]!);

    // The second option is a misconception: the continental plate is the
    // lighter one.
    await fake.user.click(
      within(group).getByRole("radio", {
        name: plateTectonicsQuestions[0]!.options[1],
      }),
    );
    await fake.user.click(screen.getByRole("button", { name: "Continue" }));
    await expectNoMarking();

    await fake.release();

    expect(await question(stems[1]!)).toBeVisible();
    expect(screen.queryByText(stems[0]!)).toBeNull();
    await expectNoMarking();
  });

  it("says what is happening while the next question is written, with the answer kept and locked", async () => {
    const fake = renderApp("empty-library", { hold: ["AnswerPlacement"] });
    await startCourse(fake.user);
    await rate(fake.user);
    const group = await question(stems[0]!);
    const chosen = within(group).getByRole("radio", {
      name: plateTectonicsQuestions[0]!.options[2],
    });
    await fake.user.click(chosen);
    // Announced only if it appears in a live region that was already there.
    const live = screen
      .getByText(/Nothing is marked right or wrong here/)
      .closest("[aria-live]");
    await fake.user.click(screen.getByRole("button", { name: "Continue" }));

    const status = await screen.findByRole("status");
    expect(status).toBe(live);
    expect(status).toHaveTextContent(
      "Choosing what comes next from your answer.",
    );
    const locked = await question(stems[0]!);
    for (const option of within(locked).getAllByRole("radio"))
      expect(option).toBeDisabled();
    expect(
      within(locked).getByRole("radio", {
        name: plateTectonicsQuestions[0]!.options[2],
      }),
    ).toBeChecked();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    await fake.user.keyboard("1");
    expect(
      within(locked).getByRole("radio", {
        name: plateTectonicsQuestions[0]!.options[2],
      }),
    ).toBeChecked();

    await fake.release();

    expect(await question(stems[1]!)).toBeVisible();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("asks a fifth and a sixth question when placement goes on, and draws them as possible until asked", async () => {
    const fake = renderApp("placement-six-questions");
    await startCourse(fake.user);
    await rate(fake.user);

    for (let number = 1; number <= 6; number += 1) {
      const group = await question(stems[number - 1]!);
      expect(screen.getByText(`Question ${number} of 4 to 6`)).toBeVisible();
      if (number === 5) {
        expect(marks()).toEqual([
          "Question 1, answered",
          "Question 2, answered",
          "Question 3, answered",
          "Question 4, answered",
          "Question 5, current",
          "Question 6, may not be asked",
        ]);
      }
      if (number === 6) {
        expect(marks()).toEqual([
          "Question 1, answered",
          "Question 2, answered",
          "Question 3, answered",
          "Question 4, answered",
          "Question 5, answered",
          "Question 6, current",
        ]);
      }
      await fake.user.click(within(group).getAllByRole("radio")[0]!);
      await fake.user.click(screen.getByRole("button", { name: "Continue" }));
    }

    expect(
      await screen.findByRole("article", { name: "Layers by composition" }),
    ).toBeVisible();
    expect(
      fake.placementRequests().filter(({ type }) => type === "AnswerPlacement"),
    ).toHaveLength(6);
  });

  it("says the questions could not be prepared, with Retry that prepares them", async () => {
    const fake = renderApp("empty-library");
    fake.core.world.placementFailure = {
      step: "PreparingQuestions",
      error: { type: "Offline" },
    };
    await startCourse(fake.user);
    await rate(fake.user);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      "The questions could not be prepared. The app is offline.",
    );
    expect(screen.queryByRole("status")).toBeNull();

    await fake.user.click(screen.getByRole("button", { name: "Retry" }));

    expect(await question(stems[0]!)).toBeVisible();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(fake.placementRequests().at(-1)).toEqual({
      type: "RetryPlacementStep",
      courseId: expect.any(String),
    });
  });

  it("says the index could not be built, keeps the failure when the course is left and opened again, and Retry builds it", async () => {
    const fake = renderApp("placement-step-fails");
    await startCourse(fake.user);
    await rate(fake.user);
    for (const stem of stems.slice(0, 4)) {
      const group = await question(stem);
      await fake.user.click(within(group).getAllByRole("radio")[4]!);
      await fake.user.click(screen.getByRole("button", { name: "Continue" }));
    }

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The index could not be built. The request failed after 4 attempts.",
    );

    await fake.user.click(screen.getByRole("button", { name: "Library" }));
    await fake.user.click(
      await screen.findByRole("button", { name: "Plate tectonics" }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The index could not be built.",
    );
    await fake.user.click(screen.getByRole("button", { name: "Retry" }));

    expect(
      await screen.findByRole("article", { name: "Layers by composition" }),
    ).toBeVisible();
    expect(
      fake
        .placementRequests()
        .filter(({ type }) => type === "RetryPlacementStep"),
    ).toHaveLength(1);
  });

  it("keeps the course when left for the library, and resumes at the next question with the answers kept", async () => {
    const fake = renderApp("default");
    await fake.user.click(
      await screen.findByRole("button", { name: "The French Revolution" }),
    );

    const second = await screen.findByRole("group", {
      name: frenchRevolutionQuestions[1]!.stem,
    });
    expect(screen.getByText("Question 2 of 4 to 6")).toBeVisible();
    await fake.user.keyboard("2");
    await fake.user.keyboard("{Enter}");
    expect(
      await screen.findByRole("group", {
        name: frenchRevolutionQuestions[2]!.stem,
      }),
    ).toBeVisible();
    expect(second).not.toBeInTheDocument();

    await fake.user.click(screen.getByRole("button", { name: "Library" }));
    expect(await screen.findByRole("textbox", { name: "Topic" })).toBeVisible();
    await fake.user.click(
      await screen.findByRole("button", { name: "The French Revolution" }),
    );

    const third = await screen.findByRole("group", {
      name: frenchRevolutionQuestions[2]!.stem,
    });
    expect(third).toHaveFocus();
    expect(screen.getByText("Question 3 of 4 to 6")).toBeVisible();
    expect(marks().slice(0, 3)).toEqual([
      "Question 1, answered",
      "Question 2, answered",
      "Question 3, current",
    ]);
    const answers = fake.answersSent();
    expect(answers.map((request) => request.answer)).toEqual([
      { type: "Option", index: 1 },
    ]);
  });
});
