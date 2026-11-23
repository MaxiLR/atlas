import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";

import { App } from "@/App";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import { drained, startedAt } from "@/test/fake-core";
import { endAnimation } from "@/test/styles";

beforeAll(() => {
  // Radix measures floating content; jsdom has no layout to observe.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

/** A stand-in for the system colour scheme that a test can flip. */
interface SystemScheme {
  set(scheme: "light" | "dark"): void;
  restore(): void;
}

function emulateSystemScheme(initial: "light" | "dark"): SystemScheme {
  let dark = initial === "dark";
  const listeners = new Set<() => void>();
  const original = window.matchMedia;
  window.matchMedia = ((query: string) => ({
    get matches() {
      return query.includes("dark") ? dark : !dark;
    },
    media: query,
    addEventListener: (_: string, listener: () => void) =>
      listeners.add(listener),
    removeEventListener: (_: string, listener: () => void) =>
      listeners.delete(listener),
  })) as unknown as typeof window.matchMedia;
  return {
    set(scheme: "light" | "dark") {
      dark = scheme === "dark";
      act(() => listeners.forEach((listener) => listener()));
    },
    restore() {
      window.matchMedia = original;
    },
  };
}

let system: SystemScheme;

beforeEach(() => {
  localStorage.clear();
  delete document.documentElement.dataset.theme;
  system = emulateSystemScheme("light");
});

afterEach(() => system.restore());

const pageTheme = () => document.documentElement.dataset.theme;

/**
 * The app on the fake core in `scenario`. Every request reaches the fake
 * core and is recorded in `requests`, except those named in `refuse`, which
 * fail with their error.
 */
function renderApp(
  scenario: string,
  refuse: Partial<Record<CoreRequest["type"], CoreError>> = {},
) {
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  const requests: CoreRequest[] = [];
  const client: CoreClient = {
    call: ((request: CoreRequest) => {
      requests.push(request);
      const error = refuse[request.type];
      return error === undefined ? core.call(request) : Promise.reject(error);
    }) as CoreClient["call"],
    subscribe: (listener) => core.subscribe(listener),
  };
  const tree = (
    <CoreProvider client={client}>
      <App />
    </CoreProvider>
  );
  const rendered = render(tree);
  return {
    core,
    requests,
    user: userEvent.setup(),
    /** Closes the window and opens it again on the same core. */
    relaunch() {
      rendered.unmount();
      render(tree);
    },
  };
}

/** Opens the app in `scenario` and goes from the library to Settings. */
async function openSettings(
  scenario = "default",
  refuse: Partial<Record<CoreRequest["type"], CoreError>> = {},
) {
  const app = renderApp(scenario, refuse);
  await app.user.click(await screen.findByRole("button", { name: "Settings" }));
  await screen.findByRole("heading", { name: "Settings", level: 1 });
  return app;
}

function account() {
  return screen.getByRole("region", { name: "Account" });
}

async function chooseTheme(user: UserEvent, name: "Light" | "Dark" | "System") {
  await user.click(screen.getByRole("button", { name: /^Theme: / }));
  const menu = await screen.findByRole("menu");
  await user.click(within(menu).getByRole("menuitemradio", { name }));
  endAnimation(menu);
}

describe("Settings", () => {
  describe("the account", () => {
    it("shows the connected account and that plan usage is on, with nothing to enable", async () => {
      await openSettings("default");

      expect(
        within(account()).getByText("candidate@example.com"),
      ).toBeVisible();
      expect(
        within(account()).getByText(
          "On. New sections are written with your ChatGPT plan.",
        ),
      ).toBeVisible();
      expect(
        screen.queryByRole("button", { name: "Enable plan usage" }),
      ).toBeNull();
    });

    it("says when plan usage is off and enables it on request", async () => {
      const { requests, user } = await openSettings("plan-usage-off");

      expect(
        within(account()).getByText(
          "Off. No course can start until plan usage is enabled.",
        ),
      ).toBeVisible();
      await user.click(
        within(account()).getByRole("button", { name: "Enable plan usage" }),
      );

      expect(requests).toContainEqual({ type: "EnablePlanUsage" });
      expect(
        await within(account()).findByText(
          "On. New sections are written with your ChatGPT plan.",
        ),
      ).toBeVisible();
      expect(
        screen.queryByRole("button", { name: "Enable plan usage" }),
      ).toBeNull();
    });

    it("says when plan usage is not allowed, with no way to enable it", async () => {
      await openSettings("plan-not-allowed");

      expect(
        within(account()).getByText(
          "Not allowed for this account or workspace.",
        ),
      ).toBeVisible();
      expect(
        screen.queryByRole("button", { name: "Enable plan usage" }),
      ).toBeNull();
      expect(
        within(account()).getByRole("button", { name: "Manage usage" }),
      ).toBeEnabled();
    });

    it("opens ChatGPT's usage settings in the browser from Manage usage", async () => {
      const { core, requests, user } = await openSettings();

      await user.click(
        within(account()).getByRole("button", { name: "Manage usage" }),
      );

      expect(requests).toContainEqual({ type: "OpenManageUsage" });
      await waitFor(() =>
        expect(core.opened).toEqual(["https://chatgpt.com/settings/usage"]),
      );
    });

    it("says in its own sentence when Manage usage could not be opened, with no notice", async () => {
      const { user } = await openSettings("default", {
        OpenManageUsage: {
          type: "Provider",
          message: "No application is set to open web links.",
        },
      });

      await user.click(
        within(account()).getByRole("button", { name: "Manage usage" }),
      );

      const line = await screen.findByRole("alert");
      expect(line).toHaveTextContent(
        "ChatGPT's usage settings could not be opened. No application is set to open web links.",
      );
      await act(drained);
      expect(screen.getAllByRole("alert")).toEqual([line]);
      expect(screen.queryByText("Link could not be opened")).toBeNull();
    });

    it("signs out to the sign-in screen and keeps the courses", async () => {
      const { requests, user } = await openSettings();

      await user.click(
        within(account()).getByRole("button", { name: "Sign out" }),
      );

      expect(requests).toContainEqual({ type: "SignOut" });
      await user.click(
        await screen.findByRole("button", { name: "Continue with ChatGPT" }),
      );
      expect(
        await screen.findByRole("button", { name: "Plate tectonics" }),
      ).toBeVisible();
    });

    it("says why a request failed and stays on Settings", async () => {
      const { user } = await openSettings("default", {
        SignOut: { type: "Offline" },
      });

      await user.click(
        within(account()).getByRole("button", { name: "Sign out" }),
      );

      expect(await screen.findByRole("alert")).toHaveTextContent(
        "The app is offline.",
      );
      expect(
        screen.getByRole("heading", { name: "Settings", level: 1 }),
      ).toBeVisible();
    });
  });

  describe("the model", () => {
    it("shows the model in use and picks another", async () => {
      const { core, requests, user } = await openSettings();

      await user.click(
        await screen.findByRole("button", { name: "Model: GPT-5.5" }),
      );
      const menu = await screen.findByRole("menu");
      await user.click(
        within(menu).getByRole("menuitemradio", { name: "GPT-5.4" }),
      );
      endAnimation(menu);

      expect(requests).toContainEqual({ type: "SelectModel", slug: "gpt-5.4" });
      expect(
        await screen.findByRole("button", { name: "Model: GPT-5.4" }),
      ).toBeVisible();
      expect(core.world.models.current).toBe("gpt-5.4");
    });
  });

  describe("the theme", () => {
    it("follows the system by default", async () => {
      await openSettings();

      expect(
        screen.getByRole("button", { name: "Theme: System" }),
      ).toBeVisible();
      expect(pageTheme()).toBe("light");
      system.set("dark");
      expect(pageTheme()).toBe("dark");
    });

    it("applies light, dark and the system's scheme at once", async () => {
      system.set("dark");
      const { user } = await openSettings();

      await chooseTheme(user, "Light");
      expect(pageTheme()).toBe("light");
      expect(
        await screen.findByRole("button", { name: "Theme: Light" }),
      ).toBeVisible();

      await chooseTheme(user, "Dark");
      expect(pageTheme()).toBe("dark");
      system.set("light");
      expect(pageTheme()).toBe("dark");

      await chooseTheme(user, "System");
      expect(pageTheme()).toBe("light");
      system.set("dark");
      expect(pageTheme()).toBe("dark");
    });

    it("keeps the choice across launches", async () => {
      const { user, relaunch } = await openSettings();

      await chooseTheme(user, "Dark");
      relaunch();

      await screen.findByRole("button", { name: "Settings" });
      expect(pageTheme()).toBe("dark");
      await user.click(screen.getByRole("button", { name: "Settings" }));
      expect(
        await screen.findByRole("button", { name: "Theme: Dark" }),
      ).toBeVisible();
    });

    it("marks the mode in use in its menu", async () => {
      const { user } = await openSettings();

      await chooseTheme(user, "Dark");
      await user.click(screen.getByRole("button", { name: "Theme: Dark" }));

      const items = within(await screen.findByRole("menu")).getAllByRole(
        "menuitemradio",
      );
      expect(items.map((item) => item.textContent)).toEqual([
        "Light",
        "Dark",
        "System",
      ]);
      expect(items.map((item) => item.getAttribute("aria-checked"))).toEqual([
        "false",
        "true",
        "false",
      ]);
    });
  });

  describe("the way back", () => {
    it("returns to the library from its button", async () => {
      const { user } = await openSettings();

      await user.click(screen.getByRole("button", { name: "Library" }));

      expect(
        await screen.findByRole("textbox", { name: "Topic" }),
      ).toBeVisible();
    });

    it("returns to the library on Escape", async () => {
      const { user } = await openSettings();

      await user.keyboard("{Escape}");

      expect(
        await screen.findByRole("textbox", { name: "Topic" }),
      ).toBeVisible();
    });

    it("lets Escape close an open menu without leaving", async () => {
      const { user } = await openSettings();
      await user.click(screen.getByRole("button", { name: /^Theme: / }));
      await screen.findByRole("menu");

      await user.keyboard("{Escape}");

      await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
      expect(
        screen.getByRole("heading", { name: "Settings", level: 1 }),
      ).toBeVisible();
    });
  });
});
