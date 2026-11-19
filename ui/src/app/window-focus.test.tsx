import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { App } from "@/App";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import { drained, startedAt } from "@/test/fake-core";

/** The window focus changes the fake core was told of, in order. */
function renderApp() {
  const core = createFakeCore({
    scenario: "default",
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  const focus: boolean[] = [];
  const client: CoreClient = {
    call(request: CoreRequest) {
      if (request.type === "SetWindowFocused") focus.push(request.focused);
      return core.call(request);
    },
    subscribe: (listener) => core.subscribe(listener),
  };
  render(
    <CoreProvider client={client}>
      <App />
    </CoreProvider>,
  );
  return focus;
}

describe("the window's focus", () => {
  it("reaches the core as SetWindowFocused when the window gains and loses it", async () => {
    const focus = renderApp();
    await screen.findByRole("textbox", { name: "Topic" });
    const before = focus.length;

    await act(async () => {
      fireEvent.blur(window);
      await drained();
    });
    expect(focus.slice(before)).toEqual([false]);

    await act(async () => {
      fireEvent.focus(window);
      await drained();
    });
    expect(focus.slice(before)).toEqual([false, true]);
  });

  it("tells the core whether the window has focus when the interface starts", async () => {
    const focus = renderApp();
    await screen.findByRole("textbox", { name: "Topic" });

    expect(focus[0]).toBe(document.hasFocus());
  });
});
