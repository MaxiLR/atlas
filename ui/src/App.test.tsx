import { render, screen, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { App } from "./App";
import { CoreProvider } from "./core/context";
import { createFakeCore, type FakeScript } from "./core/fake";
import { createGate } from "./test/gate";

const script: FakeScript = {
  chapters: [
    {
      title: "Plates",
      sections: [
        { title: "Crust and mantle", summary: "What the plates are." },
        { title: "Plate boundaries", summary: "Where plates meet." },
      ],
    },
    {
      title: "Motion",
      sections: [{ title: "Convection", summary: "What moves them." }],
    },
  ],
  sectionDeltas: ["The crust ", "floats on ", "the mantle."],
};

function renderApp() {
  const gate = createGate();
  const core = createFakeCore({ script, pace: gate.wait });
  render(
    <CoreProvider client={core}>
      <App />
    </CoreProvider>,
  );
  return { gate, user: userEvent.setup() };
}

async function startCourse(user: UserEvent) {
  await user.type(
    screen.getByRole("textbox", { name: "Topic" }),
    "Plate tectonics",
  );
  await user.click(screen.getByRole("button", { name: "Start course" }));
  return screen.findByRole("navigation", { name: "Index" });
}

describe("the core loop against the fake core", () => {
  it("shows the index once a topic is submitted", async () => {
    const { user } = renderApp();

    const index = await startCourse(user);

    expect(
      screen.getByRole("heading", { name: "Plate tectonics" }),
    ).toBeVisible();
    expect(within(index).getByText("Plates")).toBeVisible();
    expect(within(index).getByText("Motion")).toBeVisible();
    expect(
      within(index)
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual(["Crust and mantle", "Plate boundaries", "Convection"]);
  });

  it("shows a section's text growing as it streams", async () => {
    const { gate, user } = renderApp();
    const index = await startCourse(user);

    await user.click(
      within(index).getByRole("button", { name: "Crust and mantle" }),
    );
    const section = await screen.findByRole("article", {
      name: "Crust and mantle",
    });
    expect(section).not.toHaveTextContent("The crust");

    gate.step();
    await expect.poll(() => section.textContent).toContain("The crust");
    expect(section).not.toHaveTextContent("floats on");

    gate.step();
    await expect
      .poll(() => section.textContent)
      .toContain("The crust floats on");
    expect(section).not.toHaveTextContent("the mantle.");

    gate.step();
    await expect
      .poll(() => section.textContent)
      .toContain("The crust floats on the mantle.");
  });

  it("opens the following section with Next", async () => {
    const { gate, user } = renderApp();
    const index = await startCourse(user);
    await user.click(
      within(index).getByRole("button", { name: "Crust and mantle" }),
    );
    gate.step();
    gate.step();
    gate.step();
    const first = await screen.findByRole("article", {
      name: "Crust and mantle",
    });
    await expect
      .poll(() => first.textContent)
      .toContain("The crust floats on the mantle.");

    await user.click(screen.getByRole("button", { name: "Next" }));

    const second = await screen.findByRole("article", {
      name: "Plate boundaries",
    });
    expect(
      screen.queryByRole("article", { name: "Crust and mantle" }),
    ).not.toBeInTheDocument();
    gate.step();
    gate.step();
    gate.step();
    await expect
      .poll(() => second.textContent)
      .toContain("The crust floats on the mantle.");
  });
});
