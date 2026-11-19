import { expect, test, type Locator, type Page } from "@playwright/test";

import { openApp } from "./support/app.ts";

/** One step of the fake core's pace in Vite mode `fake`. */
const STEP = 80;
const ORIGINAL_TEXT = "Plates move toward each other at a few centimetres";
const EXAMPLES_TEXT = "Take an example first.";

/**
 * Opens Convergent boundaries of Plate tectonics (one stored version) with
 * the page's clock installed, so a test can hold the fake core's pace.
 */
async function openSection(page: Page, colorScheme: "light" | "dark" = "dark") {
  await page.clock.install();
  await openApp(page, { colorScheme });
  await page.getByRole("button", { name: "Plate tectonics" }).click();
  await page
    .getByRole("navigation", { name: "Index" })
    .getByRole("button", { name: "Convergent boundaries" })
    .click();
  const article = page.getByRole("article", { name: "Convergent boundaries" });
  await expect(article).toContainText(ORIGINAL_TEXT);
  return article;
}

/** Holds the page's clock, so the fake core takes a step only on `runFor`. */
async function holdClock(page: Page) {
  const now = await page.evaluate(() => Date.now());
  await page.clock.pauseAt(now + 1_000);
}

const versionSwitch = (page: Page, name: string) =>
  page.getByRole("button", { name: `Version: ${name}` });

/** The text an element shows on screen, its whitespace collapsed. */
async function shownText(locator: Locator) {
  return (await locator.innerText()).replace(/\s+/g, " ").trim();
}
for (const scheme of ["light", "dark"] as const) {
  test(`More examples streams in as a second version, and the switch moves between versions without a new generation (${scheme})`, async ({
    page,
  }) => {
    const article = await openSection(page, scheme);
    await expect(page.getByRole("button", { name: /^Version/ })).toHaveCount(0);
    await holdClock(page);

    await article.getByRole("button", { name: "More examples" }).click();
    await expect(article.getByRole("status")).toContainText(
      "Searching the web",
    );
    // One step for its web search, one for the first words.
    await page.clock.runFor(2 * STEP);
    await expect(article).toContainText(EXAMPLES_TEXT);
    await expect(article.getByRole("status")).toContainText(
      "Writing this section",
    );
    for (const name of ["Simpler", "More technical", "More examples"]) {
      await expect(
        article.getByRole("button", { name, disabled: false }),
      ).toHaveCount(0);
    }

    await page.clock.resume();
    const examples = versionSwitch(page, "More examples, 2 of 2");
    await expect(examples).toBeVisible();
    await expect(examples).toContainText("2 of 2");
    await expect(examples).toContainText("More examples");

    // The clock held again: a version that needed generating could not appear.
    await holdClock(page);
    await examples.click();
    await page.getByRole("menuitemradio", { name: /Original/ }).click();
    await expect(versionSwitch(page, "Original, 1 of 2")).toBeVisible();
    await expect(article).toContainText(ORIGINAL_TEXT);
    await expect(article).not.toContainText(EXAMPLES_TEXT);

    await versionSwitch(page, "Original, 1 of 2").click();
    await page.getByRole("menuitemradio", { name: /More examples/ }).click();
    await expect(versionSwitch(page, "More examples, 2 of 2")).toBeVisible();
    await expect(article).toContainText(EXAMPLES_TEXT);
    await expect(article.getByRole("status")).toHaveCount(0);
  });
}

test("below 1024 px the switch shows only its count, its name kept for assistive technology", async ({
  page,
}) => {
  const article = await openSection(page);
  await article.getByRole("button", { name: "More examples" }).click();
  const examples = versionSwitch(page, "More examples, 2 of 2");
  await expect(examples).toBeVisible();
  expect(await shownText(examples)).toBe("More examples 2 of 2");

  await page.setViewportSize({ width: 720, height: 900 });

  await expect(examples).toBeVisible();
  await expect.poll(() => shownText(examples)).toBe("2 of 2");
  await expect(examples).toHaveAccessibleName("Version: More examples, 2 of 2");
});

test("the version menu works from the keyboard", async ({ page }) => {
  const article = await openSection(page);
  await article.getByRole("button", { name: "More examples" }).click();
  const examples = versionSwitch(page, "More examples, 2 of 2");
  await expect(examples).toBeVisible();

  await examples.focus();
  await page.keyboard.press("Enter");
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitemradio")).toHaveText([
    /Original/,
    /More examples/,
  ]);
  await expect(
    menu.getByRole("menuitemradio", { name: /More examples/ }),
  ).toHaveAttribute("aria-checked", "true");

  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(examples).toBeFocused();

  await page.keyboard.press("ArrowDown");
  await expect(menu).toBeVisible();
  await page.keyboard.press("Home");
  await expect(
    menu.getByRole("menuitemradio", { name: /Original/ }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(menu).toHaveCount(0);
  const original = versionSwitch(page, "Original, 1 of 2");
  await expect(original).toBeFocused();
  await expect(article).toContainText(ORIGINAL_TEXT);
  await expect(article).not.toContainText(EXAMPLES_TEXT);

  await page.keyboard.press("Enter");
  await page.keyboard.press("End");
  await expect(
    menu.getByRole("menuitemradio", { name: /More examples/ }),
  ).toBeFocused();
  await page.keyboard.press(" ");
  await expect(versionSwitch(page, "More examples, 2 of 2")).toBeFocused();
  await expect(article).toContainText(EXAMPLES_TEXT);
});
