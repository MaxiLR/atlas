import { expect, test } from "@playwright/test";

import { openApp } from "./support/app.ts";

const width = 720;

for (const colorScheme of ["light", "dark"] as const) {
  test.describe(`signing in at ${width} px, ${colorScheme}`, () => {
    test("first launch signs in by keyboard and lands in the library", async ({
      page,
    }) => {
      await openApp(page, { scenario: "first-launch", width, colorScheme });

      const signIn = page.getByRole("button", {
        name: "Continue with ChatGPT",
      });
      await expect(signIn).toBeVisible();
      await expect(page.getByRole("button")).toHaveCount(1);
      await page.keyboard.press("Tab");
      await expect(signIn).toBeFocused();
      await page.keyboard.press("Enter");

      await expect(page.getByRole("textbox", { name: "Topic" })).toBeVisible();
      await expect(signIn).toHaveCount(0);
    });

    test("with plan usage off, the offer enables it and a course can start", async ({
      page,
    }) => {
      await openApp(page, { scenario: "plan-usage-off", width, colorScheme });

      await page.getByRole("button", { name: "Enable plan usage" }).click();
      await expect(
        page.getByRole("button", { name: "Enable plan usage" }),
      ).toHaveCount(0);

      await page.getByRole("textbox", { name: "Topic" }).fill("Volcanoes");
      await page.getByRole("button", { name: "Start course" }).click();
      await expect(
        page.getByRole("group", { name: "How well do you know it?" }),
      ).toBeVisible();
    });

    test("with plan usage not allowed, it is explained and sign-in does not loop", async ({
      page,
    }) => {
      await openApp(page, { scenario: "plan-not-allowed", width, colorScheme });

      await expect(
        page.getByText(/not allowed for this account or workspace/),
      ).toBeVisible();
      await expect(page.getByRole("textbox", { name: "Topic" })).toBeVisible();
      await expect(
        page.getByRole("button", { name: "Continue with ChatGPT" }),
      ).toHaveCount(0);
      await expect(
        page.getByRole("button", { name: "Enable plan usage" }),
      ).toHaveCount(0);
    });

    test("a revoked login returns to sign-in, and the courses are still there after", async ({
      page,
    }) => {
      await openApp(page, { scenario: "sign-in", width, colorScheme });

      await expect(
        page.getByText(
          "Your ChatGPT login ended. Sign in again; your courses are kept.",
        ),
      ).toBeVisible();
      await expect(page.getByRole("button")).toHaveCount(1);
      await page.getByRole("button", { name: "Continue with ChatGPT" }).click();

      await expect(page.getByRole("textbox", { name: "Topic" })).toBeVisible();
      await expect(
        page.getByText("Plate tectonics", { exact: true }).first(),
      ).toBeVisible();
      await expect(
        page.getByText("The French Revolution", { exact: true }).first(),
      ).toBeVisible();
    });

    test("a returned sign-in without plan permission offers to enable it, then a course can start", async ({
      page,
    }) => {
      await openApp(page, { scenario: "plan-usage-off", width, colorScheme });
      // Sign out and back in: the plan login returns without permission again.
      await page.getByRole("button", { name: "Settings" }).click();
      await page.getByRole("button", { name: "Sign out" }).click();
      await page.getByRole("button", { name: "Continue with ChatGPT" }).click();

      await expect(
        page.getByRole("heading", { level: 1, name: "Plan usage is off" }),
      ).toBeVisible();
      await expect(
        page.getByRole("button", { name: "Continue with ChatGPT" }),
      ).toHaveCount(0);
      await page.getByRole("button", { name: "Enable plan usage" }).click();

      await page.getByRole("textbox", { name: "Topic" }).fill("Volcanoes");
      await page.getByRole("button", { name: "Start course" }).click();
      await expect(
        page.getByRole("group", { name: "How well do you know it?" }),
      ).toBeVisible();
    });
  });
}
