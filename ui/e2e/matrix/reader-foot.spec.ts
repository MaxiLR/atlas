import { expect, test, type Page, type TestInfo } from "@playwright/test";

import { openApp } from "../support/app.ts";

/**
 * Opens Plate tectonics at the project's width and theme, with the page's
 * clock installed so a test can hold the fake core's pace.
 */
async function openCourse(page: Page, testInfo: TestInfo) {
  const { viewport, colorScheme } = testInfo.project.use;
  await page.clock.install();
  await openApp(page, {
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

const foot = (page: Page) => page.locator("main footer");
const next = (page: Page) =>
  page.getByRole("button", { name: "Next", exact: true });
const goDeeper = (page: Page) =>
  page.getByRole("button", { name: "Go deeper", exact: true });

test("Next walks two sections of the course, naming the next one on the line", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo);
  await index.getByRole("button", { name: "Divergent boundaries" }).click();
  await expect(
    page.getByRole("article", { name: "Divergent boundaries" }),
  ).toContainText("widens the Atlantic");
  await expect(foot(page)).toContainText(
    "Next on the line: Convergent boundaries",
  );

  await next(page).click();
  await expect(
    page.getByRole("article", { name: "Convergent boundaries" }),
  ).toBeVisible();
  await expect(foot(page)).toContainText(
    "Next on the line: Transform boundaries",
  );
  await expect(foot(page)).toContainText(
    "Or branch here: 3 of 3 deep dives unread",
  );

  await next(page).click();
  const third = page.getByRole("article", { name: "Transform boundaries" });
  await expect(third).toContainText("we feel as earthquakes.");
  await expect(foot(page)).toContainText("Next on the line: Mantle convection");
  await expect(next(page)).toBeEnabled();
});

test("Go deeper shows the proposals in the index and their count in the foot, then opens the first", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo);
  await index.getByRole("button", { name: "Divergent boundaries" }).click();
  await expect(foot(page)).toContainText("Or branch here: no deep dives yet");

  await goDeeper(page).click();

  for (const title of [
    "Divergent boundaries: in the field",
    "Divergent boundaries: measured",
    "Divergent boundaries: open questions",
  ]) {
    await expect(index.getByRole("button", { name: title })).toHaveCount(1);
  }
  await expect(foot(page)).toContainText(
    "Or branch here: 3 of 3 deep dives unread",
  );

  await goDeeper(page).click();
  await expect(
    page.getByRole("article", { name: "Divergent boundaries: in the field" }),
  ).toContainText("This deep dive looks closer at divergent boundaries.");
  await expect(foot(page)).toContainText(
    "Next on the line: Divergent boundaries: measured",
  );
});

test("from the last deep dive, Next returns to the broad pass", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo);
  await index.getByRole("button", { name: "Continental collision" }).click();
  await expect(
    page.getByRole("article", { name: "Continental collision" }),
  ).toContainText("like the Himalaya.");
  await expect(foot(page)).toContainText(
    "Next on the line: Transform boundaries",
  );

  await next(page).click();

  await expect(
    page.getByRole("article", { name: "Transform boundaries" }),
  ).toContainText("we feel as earthquakes.");
});

test("Go deeper and Next work from the keyboard, keeping focus while a section generates", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo);
  await index.getByRole("button", { name: "Convergent boundaries" }).click();
  await expect(foot(page)).toContainText(
    "Or branch here: 3 of 3 deep dives unread",
  );

  await goDeeper(page).focus();
  await page.keyboard.press("Enter");
  const first = page.getByRole("article", {
    name: "Subduction, step by step",
  });
  await expect(first).toBeVisible();
  await expect(goDeeper(page)).toBeFocused();

  await page.keyboard.press("Tab");
  await expect(next(page)).toBeFocused();
  await expect(first).toContainText("feeding the volcanic arc.");
  await expect(next(page)).toBeEnabled();
  await page.keyboard.press("Enter");

  const second = page.getByRole("article", {
    name: "Why the denser plate sinks",
  });
  await expect(second).toContainText("the colder and heavier it gets.");
  await expect(next(page)).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("article", { name: "Continental collision" }),
  ).toBeVisible();
});

test("the foot keeps to one row at the window's width; below 1024 px the drawn line is hidden", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo);
  await index.getByRole("button", { name: "Convergent boundaries" }).click();
  await expect(foot(page)).toContainText(
    "Next on the line: Transform boundaries",
  );
  const width = page.viewportSize()!.width;

  const line = foot(page).locator("svg").first();
  if (width < 1024) await expect(line).toBeHidden();
  else await expect(line).toBeVisible();

  const box = (await foot(page).boundingBox())!;
  expect(box.height).toBeCloseTo(64, 0);
  const deeper = (await goDeeper(page).boundingBox())!;
  const nextBox = (await next(page).boundingBox())!;
  expect(nextBox.x + nextBox.width).toBeLessThanOrEqual(width);
  expect(deeper.x + deeper.width).toBeLessThanOrEqual(nextBox.x);
  for (const text of [
    "Next on the line: Transform boundaries",
    "Or branch here: 3 of 3 deep dives unread",
  ]) {
    const shown = (await foot(page).getByText(text).boundingBox())!;
    // One line each, ending before Go deeper.
    expect(shown.height).toBeLessThan(24);
    expect(shown.x + shown.width).toBeLessThanOrEqual(deeper.x);
  }
});

test("while deep dives are requested the branch draws, and under reduced motion it is already drawn", async ({
  page,
}, testInfo) => {
  const index = await openCourse(page, testInfo);
  await index.getByRole("button", { name: "Divergent boundaries" }).click();
  await expect(foot(page)).toContainText("Or branch here: no deep dives yet");
  const line = foot(page).locator("svg").first();
  const animations = () =>
    line.evaluate((svg) => svg.getAnimations({ subtree: true }).length);

  await holdClock(page);
  await goDeeper(page).click();
  await expect(foot(page)).toContainText(
    "Or branch here: proposing deep dives…",
  );
  await expect(goDeeper(page)).toHaveAttribute("aria-busy", "true");
  if (page.viewportSize()!.width >= 1024) {
    expect(await animations()).toBeGreaterThan(0);
  }

  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.clock.resume();
  await expect(foot(page)).toContainText(
    "Or branch here: 3 of 3 deep dives unread",
  );
  await index
    .getByRole("button", { name: "Transform boundaries", exact: true })
    .click();
  await expect(
    page.getByRole("article", { name: "Transform boundaries" }),
  ).toContainText("we feel as earthquakes.");
  await holdClock(page);
  await goDeeper(page).click();
  await expect(foot(page)).toContainText(
    "Or branch here: proposing deep dives…",
  );
  expect(await animations()).toBe(0);
});
