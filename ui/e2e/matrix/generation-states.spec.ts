import { expect, test, type Page, type TestInfo } from "@playwright/test";

import { openApp } from "../support/app.ts";

/** One step of the fake core's pace in Vite mode `fake`. */
const STEP = 80;

/**
 * Opens Plate tectonics in `scenario` at the project's width and theme, with
 * the page's clock installed so a test can hold the fake core's pace.
 */
async function openCourse(page: Page, testInfo: TestInfo, scenario: string) {
  const { viewport, colorScheme } = testInfo.project.use;
  await page.clock.install();
  await openApp(page, {
    scenario,
    width: viewport?.width,
    colorScheme: colorScheme ?? undefined,
  });
  await page.getByRole("button", { name: "Plate tectonics" }).click();
  return page.getByRole("navigation", { name: "Index" });
}

/** Holds the page's clock, so the fake core takes a step only on `runFor`. */
async function holdClock(page: Page) {
  const now = await page.evaluate(() => Date.now());
  await page.clock.pauseAt(now + 1_000);
}

test("Stop marks a streaming section incomplete with Retry, and Retry completes it, by keyboard", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo, "default");
  await holdClock(page);

  await index.getByRole("button", { name: "Transform boundaries" }).click();
  const article = page.getByRole("article", { name: "Transform boundaries" });
  await expect(article.getByRole("status")).toContainText(
    "Writing this section",
  );

  const stop = article.getByRole("button", { name: "Stop" });
  await stop.focus();
  await page.keyboard.press("Enter");
  await page.clock.runFor(STEP);

  await expect(article).toContainText("This section is incomplete.");
  await expect(article.getByRole("button", { name: "Stop" })).toHaveCount(0);
  const retry = article.getByRole("button", { name: "Retry" });
  await expect(retry).toBeFocused();

  await page.clock.resume();
  await page.keyboard.press("Enter");
  await expect(article).toContainText("we feel as earthquakes.");
  await expect(article).not.toContainText("This section is incomplete.");
  await expect(article.getByRole("button", { name: "Retry" })).toHaveCount(0);
});

test("the plan-limit scenario says the limit is reached, when it resets, and offers Manage usage", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo, "plan-limit");

  await index.getByRole("button", { name: "Transform boundaries" }).click();
  const article = page.getByRole("article", { name: "Transform boundaries" });

  await expect(article).toContainText("The plan's usage limit is reached.");
  await expect(article).toContainText(/It resets in 2 h 1[34] min\./);
  const manage = article.getByRole("button", { name: "Manage usage" });
  await expect(manage).toBeVisible();
  await manage.focus();
  await expect(manage).toBeFocused();
});

test("offline, a stored section opens normally and an unopened one says the app is offline", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo, "offline");

  await index.getByRole("button", { name: "Convergent boundaries" }).click();
  const storedSection = page.getByRole("article", {
    name: "Convergent boundaries",
  });
  await expect(storedSection).toContainText(
    "Plates move toward each other at a few centimetres a year",
  );
  await expect(storedSection).not.toContainText("offline");

  await index.getByRole("button", { name: "Transform boundaries" }).click();
  const unopened = page.getByRole("article", { name: "Transform boundaries" });
  await expect(unopened).toContainText(
    "The app is offline. This section is written once it is back online.",
  );
  await expect(unopened.getByRole("button", { name: "Retry" })).toBeVisible();
});

test("the retry scenario shows each attempt, then the section failed with Retry", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo, "failed-after-retries");
  await holdClock(page);

  await index.getByRole("button", { name: "Transform boundaries" }).click();
  const article = page.getByRole("article", { name: "Transform boundaries" });
  const status = article.getByRole("status");
  await expect(status).toContainText("Writing this section");

  for (const attempt of [2, 3]) {
    await page.clock.runFor(STEP);
    await expect(status).toContainText(`Retrying, attempt ${attempt}`);
  }
  // The last attempt fails as soon as it starts.
  await page.clock.runFor(STEP);

  await expect(article).toContainText(
    "This section could not be written: the request failed after 4 attempts.",
  );
  await expect(article.getByRole("button", { name: "Stop" })).toHaveCount(0);
  await expect(article.getByRole("button", { name: "Retry" })).toBeVisible();
});
