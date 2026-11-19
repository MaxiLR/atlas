import { expect, test, type Page, type TestInfo } from "@playwright/test";

import { openApp } from "../support/app.ts";

/** One step of the fake core's pace in Vite mode `fake`. */
const STEP = 80;

/** The bar texts of the fake core's reading: 2h 14m and 3 days to reset. */
const FIVE_HOURS = "5-hour window, 38% used, 55% of the window elapsed";
const WEEKLY = "Weekly window, 61% used, 57% of the window elapsed";

/**
 * Opens Plate tectonics in `scenario` at the project's width and theme, with
 * the page's clock installed so a test can hold the fake core's pace.
 * Returns the usage meter.
 */
async function openMeter(page: Page, testInfo: TestInfo, scenario: string) {
  const { viewport, colorScheme } = testInfo.project.use;
  await page.clock.install();
  await openApp(page, {
    scenario,
    width: viewport?.width,
    colorScheme: colorScheme ?? undefined,
  });
  await page.getByRole("button", { name: "Plate tectonics" }).click();
  return page.getByRole("region", { name: "Plan usage" });
}

const narrow = (testInfo: TestInfo) =>
  (testInfo.project.use.viewport?.width ?? 1440) < 1024;

test("Connect usage shows the device code, then the bars", async ({
  page,
}, testInfo) => {
  const meter = await openMeter(page, testInfo, "usage-not-connected");
  await expect(meter.getByRole("img")).toHaveCount(0);
  const now = await page.evaluate(() => Date.now());
  await page.clock.pauseAt(now + 1_000);

  await meter.getByRole("button", { name: "Connect usage" }).click();

  const dialog = page.getByRole("dialog", { name: "Connect usage" });
  await expect(dialog.getByText("QWRT-7KPD")).toBeVisible();
  await expect(dialog).toContainText(
    "OpenAI's page has opened in your browser",
  );
  await expect(dialog).toContainText("Code expires in 15m");
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeVisible();

  await page.clock.runFor(STEP);

  await expect(dialog).toHaveCount(0);
  await expect(meter.getByRole("img", { name: FIVE_HOURS })).toBeVisible();
  await expect(meter.getByRole("img", { name: WEEKLY })).toBeVisible();
});

test("the stale reading keeps the bars, marked, with the reason on focus", async ({
  page,
}, testInfo) => {
  test.skip(narrow(testInfo), "below 1024 px the meter shows its bars only");
  const meter = await openMeter(page, testInfo, "usage-stale");

  await expect(meter.getByRole("img", { name: FIVE_HOURS })).toBeVisible();
  const mark = meter.getByRole("button", { name: /Out of date/ });
  await expect(mark).toBeVisible();
  await mark.focus();

  await expect(page.getByRole("tooltip")).toContainText(
    "The latest reading failed: the network could not be reached.",
  );
});

test("the popover lists the other windows the account reports", async ({
  page,
}, testInfo) => {
  test.skip(narrow(testInfo), "below 1024 px the meter shows its bars only");
  const meter = await openMeter(page, testInfo, "usage-reading");
  await expect(meter.getByText("38% · resets in 2h 14m")).toBeVisible();
  await expect(meter.getByText("61% · resets in 3d")).toBeVisible();

  await meter.getByRole("button", { name: "1 more window" }).click();

  const popover = page.getByRole("dialog", { name: "Other windows" });
  await expect(popover.getByText("30-day")).toBeVisible();
  await expect(popover.getByText("12% · resets in 17d")).toBeVisible();
  await expect(
    popover.getByRole("img", {
      name: "30-day window, 12% used, 43% of the window elapsed",
    }),
  ).toBeVisible();
});

test("below 1024 px only the bars show, and the figures appear on focus", async ({
  page,
}, testInfo) => {
  test.skip(!narrow(testInfo), "the meter folds below 1024 px");
  const meter = await openMeter(page, testInfo, "usage-reading");

  const bar = meter.getByRole("img", { name: FIVE_HOURS });
  await expect(bar).toBeVisible();
  await expect(meter.getByRole("img", { name: WEEKLY })).toBeVisible();
  await expect(meter.getByText("38% · resets in 2h 14m")).toBeHidden();
  await expect(meter.getByRole("button")).toHaveCount(0);

  await bar.focus();

  const tooltip = page.getByRole("tooltip");
  await expect(tooltip).toContainText("5-hour");
  await expect(tooltip).toContainText("38% · resets in 2h 14m");
});
