import { render, screen, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { App } from "./App";
import type { CoreClient } from "./core/client";
import { CoreProvider } from "./core/context";
import { createFakeCore, createManualClock } from "./core/fake";
import { startedAt } from "./test/fake-core";
import { createGate } from "./test/gate";

/**
 * The default scenario's core, except that the first `refusals` requests for
 * the session are refused as a core without the session feature refuses them.
 * `asked` counts the requests for the session the core has answered.
 */
function renderWithoutSession(refusals: number) {
  const core = createFakeCore({
    scenario: "default",
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  let asked = 0;
  const client: CoreClient = {
    call(request) {
      if (request.type !== "GetSession") return core.call(request);
      asked += 1;
      return asked <= refusals
        ? Promise.reject({ type: "Unavailable", feature: "session" })
        : core.call(request);
    },
    subscribe: (listener) => core.subscribe(listener),
  };
  render(
    <CoreProvider client={client}>
      <App />
    </CoreProvider>,
  );
  return { user: userEvent.setup(), asked: () => asked };
}

/** The notice saying the session could not be read. */
async function findSessionNotice() {
  const notice = await screen.findByRole("alert");
  expect(notice).toHaveTextContent("Session could not be read");
  return notice;
}

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
    expect(
      within(index).getByRole("button", { name: "What moves the plates" }),
    ).toHaveAccessibleDescription("Chapter, 2 sections");
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

describe("a core that cannot say the session", () => {
  it("says so in a notice with Retry, and still opens the library and its courses", async () => {
    const { user } = renderWithoutSession(Infinity);

    const notice = await findSessionNotice();
    expect(notice).toHaveTextContent("session is not available yet.");
    expect(
      within(notice).getByRole("button", { name: "Retry" }),
    ).toBeInTheDocument();
    // The notice is Sonner's, and the window has no banner of its own.
    expect(
      screen.getByRole("region", { name: /^Notifications/ }),
    ).toContainElement(notice);
    expect(screen.getAllByRole("alert")).toEqual([notice]);
    expect(screen.getAllByRole("button", { name: "Retry" })).toHaveLength(1);

    const index = await openPlateTectonics(user);
    expect(within(index).getAllByRole("button").length).toBeGreaterThan(0);
  });

  it("closes the notice once Retry reads the session", async () => {
    const { user } = renderWithoutSession(1);
    const notice = await findSessionNotice();

    await user.click(within(notice).getByRole("button", { name: "Retry" }));

    await expect.poll(() => screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByText("Session could not be read")).toBeNull();
    expect(
      await screen.findByRole("button", { name: "Plate tectonics" }),
    ).toBeVisible();
  });

  it("keeps the notice when Retry fails again", async () => {
    const { user, asked } = renderWithoutSession(2);
    const notice = await findSessionNotice();

    await user.click(within(notice).getByRole("button", { name: "Retry" }));
    await expect.poll(asked).toBe(2);
    // Long enough for Sonner to have closed it, had the store said so.
    const { promise: settled, resolve } = Promise.withResolvers<void>();
    setTimeout(resolve, 600);
    await settled;

    expect(await findSessionNotice()).toHaveTextContent(
      "session is not available yet.",
    );
  });
});
