import { render, screen, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { App } from "@/App";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import { startedAt } from "@/test/fake-core";

function renderApp(scenario: string) {
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  render(
    <CoreProvider client={core}>
      <App />
    </CoreProvider>,
  );
  return { core, user: userEvent.setup() };
}

async function startCourse(user: UserEvent, topic: string) {
  await user.type(await screen.findByRole("textbox", { name: "Topic" }), topic);
  await user.click(screen.getByRole("button", { name: "Start course" }));
}

/** Rates, sets the goal and answers every question until placement ends. */
async function place(user: UserEvent) {
  const rating = await screen.findByRole("group", {
    name: "How well do you know it?",
  });
  await user.click(within(rating).getByRole("radio", { name: "New to this" }));
  const goal = screen.getByRole("group", { name: "What is your goal?" });
  await user.click(within(goal).getByRole("radio", { name: "Overview" }));
  await user.click(screen.getByRole("button", { name: "Continue" }));

  for (let number = 1; number <= 4; number += 1) {
    await screen.findByText(`Question ${number} of 4 to 6`);
    const options = screen.getAllByRole("radio");
    expect(options).toHaveLength(5);
    expect(options.at(-1)).toHaveAccessibleName("Not sure");
    await user.click(number === 2 ? options[4]! : options[0]!);
    await user.click(screen.getByRole("button", { name: "Continue" }));
  }
}

describe("the thin screens", () => {
  it("show one sign-in action on first launch, and the library once signed in", async () => {
    const { user } = renderApp("first-launch");

    const signIn = await screen.findByRole("button", {
      name: "Continue with ChatGPT",
    });
    expect(screen.queryByRole("textbox", { name: "Topic" })).toBeNull();
    await user.click(signIn);

    expect(await screen.findByRole("textbox", { name: "Topic" })).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Continue with ChatGPT" }),
    ).toBeNull();
  });

  it("lead from a topic to placement: a self-rating and a goal", async () => {
    const { user } = renderApp("empty-library");

    await startCourse(user, "Plate tectonics");

    const rating = await screen.findByRole("group", {
      name: "How well do you know it?",
    });
    expect(within(rating).getAllByRole("radio")).toHaveLength(4);
    for (const name of [
      "New to this",
      "Know the basics",
      "Use it regularly",
      "Expert",
    ]) {
      expect(within(rating).getByRole("radio", { name })).toBeVisible();
    }
    const goal = screen.getByRole("group", { name: "What is your goal?" });
    expect(within(goal).getAllByRole("radio")).toHaveLength(3);
    for (const name of ["Overview", "Working knowledge", "Expert"]) {
      expect(within(goal).getByRole("radio", { name })).toBeVisible();
    }
    expect(
      screen.getByRole("heading", { name: "Plate tectonics" }),
    ).toBeVisible();
  });

  it("lead from placement, one question at a time with Not sure, to the index", async () => {
    const { user } = renderApp("empty-library");
    await startCourse(user, "Plate tectonics");

    await place(user);

    const index = await screen.findByRole("navigation", { name: "Index" });
    expect(within(index).getByText("The solid Earth")).toBeVisible();
    // Three chapter rows and the sections of the first two chapters; the
    // chapter further along is a station with its section count.
    expect(within(index).getAllByRole("button")).toEqual(
      [
        "The solid Earth",
        "Layers by composition",
        "Lithosphere and asthenosphere",
        "Plates and their boundaries",
        "Divergent boundaries",
        "Convergent boundaries",
        "Transform boundaries",
        "What moves the plates",
      ].map((name) => within(index).getByRole("button", { name })),
    );
    const station = within(index).getByRole("button", {
      name: "What moves the plates",
    });
    expect(station).toHaveTextContent("What moves the plates2");
    expect(station).toHaveAccessibleDescription("Chapter, 2 sections");
  });

  it("show settings with the account, Manage usage and Sign out", async () => {
    const { core, user } = renderApp("default");

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    expect(await screen.findByText("candidate@example.com")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Manage usage" }));
    expect(core.opened).toEqual(["https://chatgpt.com/settings/usage"]);
    await user.click(screen.getByRole("button", { name: "Sign out" }));
    expect(
      await screen.findByRole("button", { name: "Continue with ChatGPT" }),
    ).toBeVisible();
  });

  it("offer to enable plan usage when it is off", async () => {
    const { user } = renderApp("plan-usage-off");

    await user.click(
      await screen.findByRole("button", { name: "Enable plan usage" }),
    );

    await startCourse(user, "Plate tectonics");
    expect(
      await screen.findByRole("group", { name: "How well do you know it?" }),
    ).toBeVisible();
  });

  it("explain once that the plan is not allowed, with no sign-in action", async () => {
    renderApp("plan-not-allowed");

    expect(
      await screen.findByText(/not allowed for this account/),
    ).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Continue with ChatGPT" }),
    ).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Enable plan usage" }),
    ).toBeNull();
  });

  it("open a course from the library on its index", async () => {
    const { user } = renderApp("default");

    await user.click(
      await screen.findByRole("button", { name: "Plate tectonics" }),
    );

    expect(
      await screen.findByRole("navigation", { name: "Index" }),
    ).toBeVisible();
  });

  it("open a course left in placement at its question", async () => {
    const { user } = renderApp("default");

    await user.click(
      await screen.findByRole("button", { name: "The French Revolution" }),
    );

    expect(await screen.findByText("Question 2 of 4 to 6")).toBeVisible();
  });
});
