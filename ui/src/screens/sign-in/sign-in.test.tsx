import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { App } from "@/App";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock, type FakeWorld } from "@/core/fake";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import { createValve, startedAt } from "@/test/fake-core";

const SIGN_IN_PAGE = "https://auth.openai.com/oauth/authorize";
const MANAGE_USAGE = "https://chatgpt.com/settings/usage";

/** Requests the screen makes on its own, as opposed to the app's start-up. */
const signInRequests: CoreRequest["type"][] = [
  "SignIn",
  "CancelSignIn",
  "EnablePlanUsage",
  "OpenManageUsage",
];

/**
 * The app on the fake core in `scenario`, after `prepare` has set the
 * world's conditions. Every request is recorded; `pace` holds the browser
 * coming back from a sign-in when a test needs to see it pending.
 */
function renderApp(
  scenario: string,
  {
    prepare,
    pace = () => Promise.resolve(),
  }: { prepare?: (world: FakeWorld) => void; pace?: () => Promise<void> } = {},
) {
  const core = createFakeCore({
    scenario,
    clock: createManualClock(startedAt),
    pace,
  });
  prepare?.(core.world);
  const requests: CoreRequest[] = [];
  const client: CoreClient = {
    call: ((request: CoreRequest) => {
      requests.push(request);
      return core.call(request);
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
    user: userEvent.setup(),
    /** The sign-in requests sent so far, by type. */
    sent: () =>
      requests
        .filter((request) => signInRequests.includes(request.type))
        .map((request) => request.type),
  };
}

/** The names of every button on the page, in document order. */
function buttons(): string[] {
  return screen
    .getAllByRole("button")
    .map((button) => button.textContent?.trim() ?? "");
}

/** Signs in from the first-launch screen; the browser returns at once. */
async function signInWith(planUsage: FakeWorld["grantedPlanUsage"]) {
  const app = renderApp("first-launch", {
    prepare: (world) => {
      world.grantedPlanUsage = planUsage;
    },
  });
  await app.user.click(
    await screen.findByRole("button", { name: "Continue with ChatGPT" }),
  );
  return app;
}

describe("the sign-in screen", () => {
  describe("signed out", () => {
    it("names the app, says what signing in does and offers one action", async () => {
      renderApp("first-launch");

      expect(
        await screen.findByRole("heading", { level: 1, name: "Atlas" }),
      ).toBeVisible();
      expect(
        screen.getByText(
          "Signing in lets Atlas write your courses on your own ChatGPT plan, with no API key.",
        ),
      ).toBeVisible();
      expect(buttons()).toEqual(["Continue with ChatGPT"]);
    });

    it("signs in through the browser and lands in the library", async () => {
      const { core, user, sent } = renderApp("first-launch");

      await user.click(
        await screen.findByRole("button", { name: "Continue with ChatGPT" }),
      );

      expect(
        await screen.findByRole("textbox", { name: "Topic" }),
      ).toBeVisible();
      expect(sent()).toEqual(["SignIn"]);
      expect(core.opened).toEqual([SIGN_IN_PAGE]);
    });
  });

  describe("signing in", () => {
    it("says the browser has opened and offers only Cancel", async () => {
      const valve = createValve();
      const { user } = renderApp("first-launch", { pace: valve.pace });

      await user.click(
        await screen.findByRole("button", { name: "Continue with ChatGPT" }),
      );

      expect(await screen.findByText(/Your browser has opened/)).toBeVisible();
      expect(buttons()).toEqual(["Cancel"]);
    });

    it("cancels back to the single sign-in action", async () => {
      const valve = createValve();
      const { user, sent } = renderApp("first-launch", { pace: valve.pace });
      await user.click(
        await screen.findByRole("button", { name: "Continue with ChatGPT" }),
      );

      await user.click(await screen.findByRole("button", { name: "Cancel" }));

      expect(
        await screen.findByRole("button", { name: "Continue with ChatGPT" }),
      ).toBeVisible();
      expect(buttons()).toEqual(["Continue with ChatGPT"]);
      expect(sent()).toEqual(["SignIn", "CancelSignIn"]);
      // The browser coming back late does not sign in after all.
      valve.open();
      await waitFor(() =>
        expect(screen.queryByRole("textbox", { name: "Topic" })).toBeNull(),
      );
    });

    it("moves keyboard focus to Cancel when the sign-in action goes", async () => {
      const valve = createValve();
      const { user } = renderApp("first-launch", { pace: valve.pace });
      await screen.findByRole("button", { name: "Continue with ChatGPT" });

      await user.tab();
      await user.keyboard("{Enter}");

      expect(
        await screen.findByRole("button", { name: "Cancel" }),
      ).toHaveFocus();
    });
  });

  describe("returned without permission to use the plan", () => {
    it("keeps the sign-in, says plan usage is off and offers to enable it", async () => {
      await signInWith("Off");

      expect(
        await screen.findByRole("heading", {
          level: 1,
          name: "Plan usage is off",
        }),
      ).toBeVisible();
      expect(screen.getByText(/candidate@example\.com/)).toBeVisible();
      expect(screen.getByText(/No course can start until/)).toBeVisible();
      expect(buttons()).toEqual(["Continue to library", "Enable plan usage"]);
      expect(
        screen.queryByRole("button", { name: "Continue with ChatGPT" }),
      ).toBeNull();
    });

    it("enables plan usage and moves on to the library", async () => {
      const { user, sent } = await signInWith("Off");

      await user.click(
        await screen.findByRole("button", { name: "Enable plan usage" }),
      );

      expect(
        await screen.findByRole("textbox", { name: "Topic" }),
      ).toBeVisible();
      expect(sent()).toEqual(["SignIn", "EnablePlanUsage"]);
    });

    it("continues to the library, where no course can start", async () => {
      const { user, sent } = await signInWith("Off");

      await user.click(
        await screen.findByRole("button", { name: "Continue to library" }),
      );

      expect(
        await screen.findByRole("textbox", { name: "Topic" }),
      ).toBeVisible();
      expect(screen.getByText(/no course can start/)).toBeVisible();
      expect(sent()).toEqual(["SignIn"]);
    });
  });

  describe("plan usage not allowed for the account or workspace", () => {
    it("explains it once with Manage usage and no sign-in action", async () => {
      await signInWith("NotAllowed");

      expect(
        await screen.findByRole("heading", {
          level: 1,
          name: "Plan usage is not allowed",
        }),
      ).toBeVisible();
      expect(
        screen.getByText(/not allowed for this account or workspace/),
      ).toBeVisible();
      expect(
        screen.getByText(/Signing in again would end the same way/),
      ).toBeVisible();
      expect(buttons()).toEqual(["Continue to library", "Manage usage"]);
    });

    it("opens the usage settings in the browser", async () => {
      const { core, user, sent } = await signInWith("NotAllowed");

      await user.click(
        await screen.findByRole("button", { name: "Manage usage" }),
      );

      expect(sent()).toEqual(["SignIn", "OpenManageUsage"]);
      expect(core.opened).toEqual([SIGN_IN_PAGE, MANAGE_USAGE]);
    });

    it("continues to the library and does not come back to sign-in", async () => {
      const { user, sent } = await signInWith("NotAllowed");

      await user.click(
        await screen.findByRole("button", { name: "Continue to library" }),
      );

      expect(
        await screen.findByRole("textbox", { name: "Topic" }),
      ).toBeVisible();
      expect(
        screen.queryByRole("button", { name: "Continue with ChatGPT" }),
      ).toBeNull();
      expect(
        screen.queryByRole("heading", { name: "Plan usage is not allowed" }),
      ).toBeNull();
      expect(sent()).toEqual(["SignIn"]);
    });
  });

  describe("signed out because the login ended", () => {
    it("says so in one line above the same single action", async () => {
      renderApp("sign-in");

      expect(
        await screen.findByText(
          "Your ChatGPT login ended. Sign in again; your courses are kept.",
        ),
      ).toBeVisible();
      expect(buttons()).toEqual(["Continue with ChatGPT"]);
    });

    it("finds the courses still in the library after signing in", async () => {
      const { user, sent } = renderApp("sign-in");

      await user.click(
        await screen.findByRole("button", { name: "Continue with ChatGPT" }),
      );

      expect(
        await screen.findByRole("textbox", { name: "Topic" }),
      ).toBeVisible();
      expect(
        (await screen.findAllByText("Plate tectonics")).length,
      ).toBeGreaterThan(0);
      expect(
        (await screen.findAllByText("The French Revolution")).length,
      ).toBeGreaterThan(0);
      expect(sent()).toEqual(["SignIn"]);
    });
  });

  describe("a failed attempt", () => {
    it("says permission was declined, with the same single action", async () => {
      renderApp("first-launch", {
        prepare: (world) => {
          world.session = { type: "SignedOut", reason: { type: "Declined" } };
        },
      });

      expect(
        await screen.findByText(
          "Permission to use your plan was declined in the browser.",
        ),
      ).toBeVisible();
      expect(buttons()).toEqual(["Continue with ChatGPT"]);
    });

    it("says the sign-in failed and why, with the same single action", async () => {
      renderApp("first-launch", {
        prepare: (world) => {
          world.session = {
            type: "SignedOut",
            reason: {
              type: "Failed",
              error: {
                type: "Provider",
                message: "The browser did not come back in time.",
              },
            },
          };
        },
      });

      expect(
        await screen.findByText(
          "Signing in did not finish. The browser did not come back in time.",
        ),
      ).toBeVisible();
      expect(buttons()).toEqual(["Continue with ChatGPT"]);
    });

    it("says the app is offline when the attempt cannot start", async () => {
      const { user, sent } = renderApp("first-launch", {
        prepare: (world) => {
          world.offline = true;
        },
      });

      await user.click(
        await screen.findByRole("button", { name: "Continue with ChatGPT" }),
      );

      expect(
        await screen.findByText(
          "Signing in did not start. The app is offline.",
        ),
      ).toBeVisible();
      expect(buttons()).toEqual(["Continue with ChatGPT"]);
      expect(sent()).toEqual(["SignIn"]);
    });
  });
});
