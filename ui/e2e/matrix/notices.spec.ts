import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page, type TestInfo } from "@playwright/test";

import { openApp } from "../support/app.ts";

/** The reason the fake core gives in `session-unreadable`. */
const REASON =
  "The secret store failed: secrets.json in the app-data folder is not valid JSON.";

/**
 * Opens the library at the project's width and theme with a core that cannot
 * read the session; returns its notice.
 */
async function openUnreadable(page: Page, testInfo: TestInfo) {
  const { viewport, colorScheme } = testInfo.project.use;
  await openApp(page, {
    scenario: "session-unreadable",
    width: viewport?.width,
    colorScheme: colorScheme ?? undefined,
  });
  return page
    .getByRole("alert")
    .filter({ hasText: "Session could not be read" });
}

test("the session notice sits at the bottom right, clear of the window bar", async ({
  page,
}, testInfo) => {
  const notice = await openUnreadable(page, testInfo);
  await expect(notice).toBeVisible();
  await expect(notice).toContainText(REASON);
  await expect(notice.getByRole("button", { name: "Retry" })).toBeVisible();
  await expect(notice.getByRole("button", { name: "Dismiss" })).toBeVisible();

  const window = page.viewportSize()!;
  // Once it has risen into place, 16 px above the window's bottom edge.
  await expect
    .poll(async () => {
      const box = await notice.boundingBox();
      return box === null ? null : window.height - (box.y + box.height);
    })
    .toBeCloseTo(16, 0);
  const box = (await notice.boundingBox())!;
  expect(window.width - (box.x + box.width)).toBeCloseTo(16, 0);
  expect(box.width).toBeLessThanOrEqual(356);
  // The window bar, with the traffic lights over its first 80 px.
  const bar = (await page.getByRole("banner").boundingBox())!;
  expect(bar.y).toBe(0);
  expect(box.y).toBeGreaterThanOrEqual(bar.y + bar.height);

  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa"])
    .analyze();
  expect(results.violations).toEqual([]);
});

test("Retry reads the session and the notice leaves", async ({
  page,
}, testInfo) => {
  const notice = await openUnreadable(page, testInfo);

  await notice.getByRole("button", { name: "Retry" }).click();

  await expect(notice).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Plate tectonics" }),
  ).toBeVisible();
});
