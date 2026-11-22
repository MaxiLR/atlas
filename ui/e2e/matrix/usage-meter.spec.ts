import { expect, test, type Page, type TestInfo } from "@playwright/test";

import { openApp } from "../support/app.ts";

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

test("signed in, the meter shows the bars with no connection step", async ({
  page,
}, testInfo) => {
  const meter = await openMeter(page, testInfo, "default");

  await expect(meter.getByRole("img", { name: FIVE_HOURS })).toBeVisible();
  await expect(meter.getByRole("img", { name: WEEKLY })).toBeVisible();
  await expect(page.getByRole("button", { name: "Connect usage" })).toHaveCount(
    0,
  );
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

test("awaiting its first reading, the meter is as tall as with a reading", async ({
  page,
  context,
}, testInfo) => {
  const awaiting = await openMeter(page, testInfo, "usage-awaiting");
  await expect(awaiting.getByRole("status")).toHaveText("Reading usage");
  await expect(awaiting.getByRole("img")).toHaveCount(0);
  const pending = await awaiting.boundingBox();

  const reading = await openMeter(
    await context.newPage(),
    testInfo,
    "usage-reading",
  );
  await expect(reading.getByRole("img", { name: FIVE_HOURS })).toBeVisible();
  const read = await reading.boundingBox();

  expect(pending?.height).toBeGreaterThan(0);
  expect(pending?.height).toBe(read?.height);
});

test("after a failed first reading, the meter offers Try again", async ({
  page,
}, testInfo) => {
  const meter = await openMeter(page, testInfo, "usage-first-read-failed");

  await expect(meter.getByRole("button", { name: "Try again" })).toBeVisible();
});
