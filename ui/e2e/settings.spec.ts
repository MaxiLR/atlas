import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { openApp } from "./support/app.ts";

const schemes = ["light", "dark"] as const;
type Scheme = (typeof schemes)[number];

const other: Record<Scheme, Scheme> = { light: "dark", dark: "light" };

/** The theme the page shows and the colour scheme its controls take. */
async function expectPageTheme(page: Page, scheme: Scheme) {
  const html = page.locator("html");
  await expect(html).toHaveAttribute("data-theme", scheme);
  await expect
    .poll(() => html.evaluate((root) => getComputedStyle(root).colorScheme))
    .toBe(scheme);
}

async function chooseTheme(page: Page, name: "Light" | "Dark" | "System") {
  await page.getByRole("button", { name: /^Theme: / }).click();
  await page.getByRole("menuitemradio", { name }).click();
  await expect(page.getByRole("menu")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: `Theme: ${name}` }),
  ).toBeVisible();
}

/** Tabs from the top of the library until Settings has focus, then opens it. */
async function openSettingsByKeyboard(page: Page) {
  const settings = page.getByRole("button", { name: "Settings" });
  await expect(settings).toBeVisible();
  for (let presses = 0; presses < 10; presses += 1) {
    if (
      await settings.evaluate((element) => element === document.activeElement)
    )
      break;
    await page.keyboard.press("Tab");
  }
  await expect(settings).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Settings", level: 1 }),
  ).toBeVisible();
}

for (const scheme of schemes) {
  test.describe(`at 720 px in a ${scheme} system`, () => {
    test.beforeEach(async ({ page }) => {
      await openApp(page, { width: 720, colorScheme: scheme });
    });

    test("Settings opens from the library by keyboard, switches the theme, picks a model and signs out", async ({
      page,
    }) => {
      await openSettingsByKeyboard(page);
      await expectPageTheme(page, scheme);
      await expect(
        page.getByRole("button", { name: "Theme: System" }),
      ).toBeVisible();

      await chooseTheme(page, "Light");
      await expectPageTheme(page, "light");
      await chooseTheme(page, "Dark");
      await expectPageTheme(page, "dark");
      await chooseTheme(page, "System");
      await expectPageTheme(page, scheme);
      await page.emulateMedia({ colorScheme: other[scheme] });
      await expectPageTheme(page, other[scheme]);
      await page.emulateMedia({ colorScheme: scheme });
      await expectPageTheme(page, scheme);

      await page.getByRole("button", { name: "Model: GPT-5.5" }).click();
      await page.getByRole("menuitemradio", { name: "GPT-5.4 mini" }).click();
      await expect(
        page.getByRole("button", { name: "Model: GPT-5.4 mini" }),
      ).toBeVisible();

      await page.getByRole("button", { name: "Sign out" }).click();
      await expect(
        page.getByRole("button", { name: "Continue with ChatGPT" }),
      ).toBeVisible();
    });

    test("the theme chosen holds after the window reopens", async ({
      page,
    }) => {
      await openSettingsByKeyboard(page);

      await chooseTheme(page, other[scheme] === "light" ? "Light" : "Dark");
      await page.reload();

      await expect(
        page.getByRole("button", { name: "Settings" }),
      ).toBeVisible();
      await expectPageTheme(page, other[scheme]);
    });

    test("Escape returns to the library", async ({ page }) => {
      await openSettingsByKeyboard(page);

      await page.keyboard.press("Escape");

      await expect(page.getByRole("textbox", { name: "Topic" })).toBeVisible();
    });

    test("every row fits the column and nothing scrolls sideways", async ({
      page,
    }) => {
      await openSettingsByKeyboard(page);
      await expect(page.getByText("candidate@example.com")).toBeVisible();

      const overflow = await page.evaluate(
        () =>
          document.documentElement.scrollWidth -
          document.documentElement.clientWidth,
      );
      expect(overflow).toBe(0);
      for (const name of ["Sign out", "Manage usage"]) {
        await expect(page.getByRole("button", { name })).toBeInViewport();
      }
    });

    test("has no detectable accessibility violations", async ({ page }) => {
      await openSettingsByKeyboard(page);
      await expect(page.getByText("candidate@example.com")).toBeVisible();
      await expect(
        page.getByRole("button", { name: "Model: GPT-5.5" }),
      ).toBeVisible();

      const scan = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa"])
        .analyze();
      expect(scan.violations).toEqual([]);
    });
  });
}
