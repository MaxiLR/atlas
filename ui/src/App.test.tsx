import { render, screen, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { App } from "./App";
import { CoreProvider } from "./core/context";
import { createFakeCore, createManualClock } from "./core/fake";
import { startedAt } from "./test/fake-core";
import { createGate } from "./test/gate";

function renderApp(scenario: string, pace: () => Promise<void>) {
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace,
  });
  render(
    <CoreProvider client={core}>
      <App />
    </CoreProvider>,
  );
  return userEvent.setup();
}

/** Opens Plate tectonics from the library; its index is shown. */
async function openPlateTectonics(user: UserEvent) {
  await user.click(
    await screen.findByRole("button", { name: "Plate tectonics" }),
  );
  return screen.findByRole("navigation", { name: "Index" });
}

describe("the core loop against the fake core", () => {
  it("shows the index once a topic is placed", async () => {
    const user = renderApp("empty-library", () => Promise.resolve());

    await user.type(
      await screen.findByRole("textbox", { name: "Topic" }),
      "Plate tectonics",
    );
    await user.click(screen.getByRole("button", { name: "Start course" }));
    await user.click(await screen.findByRole("radio", { name: "New to this" }));
    await user.click(screen.getByRole("radio", { name: "Overview" }));
    await user.click(screen.getByRole("button", { name: "Continue" }));
    for (let number = 1; number <= 4; number += 1) {
      await screen.findByText(`Question ${number} of 4 to 6`);
      await user.click(screen.getByRole("radio", { name: "Not sure" }));
      await user.click(screen.getByRole("button", { name: "Continue" }));
    }

    const index = await screen.findByRole("navigation", { name: "Index" });
    expect(
      screen.getByRole("heading", { name: "Plate tectonics" }),
    ).toBeVisible();
    expect(within(index).getByText("The solid Earth")).toBeVisible();
    expect(within(index).getByText("What moves the plates")).toBeVisible();
    expect(within(index).getAllByRole("button")).toHaveLength(7);
  });

  it("shows a section's text growing as it streams", async () => {
    const gate = createGate();
    const user = renderApp("default", gate.wait);
    const index = await openPlateTectonics(user);

    await user.click(
      within(index).getByRole("button", { name: "Transform boundaries" }),
    );
    const section = await screen.findByRole("article", {
      name: "Transform boundaries",
    });
    expect(section).not.toHaveTextContent("Where plates slide");

    gate.step();
    await expect
      .poll(() => section.textContent)
      .toContain("Where plates slide past each other");
    expect(section).not.toHaveTextContent("San Andreas");

    gate.step();
    await expect.poll(() => section.textContent).toContain("San Andreas");
    expect(section).not.toHaveTextContent("earthquakes.");

    gate.step();
    await expect.poll(() => section.textContent).toContain("earthquakes.");
  });

  it("opens the following section with Next", async () => {
    const gate = createGate();
    const user = renderApp("default", gate.wait);
    const index = await openPlateTectonics(user);
    await user.click(
      within(index).getByRole("button", { name: "Transform boundaries" }),
    );
    gate.step();
    gate.step();
    gate.step();
    const first = await screen.findByRole("article", {
      name: "Transform boundaries",
    });
    await expect.poll(() => first.textContent).toContain("earthquakes.");

    await user.click(screen.getByRole("button", { name: "Next" }));

    const second = await screen.findByRole("article", {
      name: "Mantle convection",
    });
    expect(
      screen.queryByRole("article", { name: "Transform boundaries" }),
    ).not.toBeInTheDocument();
    gate.step();
    gate.step();
    gate.step();
    await expect
      .poll(() => second.textContent)
      .toContain("like soup on a stove");
  });
});
