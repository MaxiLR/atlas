import { expect, test } from "@playwright/test";

import { openApp } from "../support/app.ts";

const FIRST =
  "A plate carrying young, warm ocean floor meets a plate carrying old, cold ocean floor. What happens next?";
const SECOND =
  "Why does a collision between two continents build mountains instead of a trench?";

test("the check block keeps one height on every question, and the text below it stays put", async ({
  page,
}, testInfo) => {
  const { viewport, colorScheme } = testInfo.project.use;
  await openApp(page, {
    scenario: "check-questions",
    width: viewport?.width,
    colorScheme: colorScheme ?? undefined,
  });
  await page.getByRole("button", { name: "Plate boundaries" }).click();
  await page
    .getByRole("navigation", { name: "Index" })
    .getByRole("button", { name: "Convergent boundaries" })
    .click();
  const article = page.getByRole("article", { name: "Convergent boundaries" });
  const block = article.getByRole("region", { name: "Check questions" });
  const after = article.getByText("Trenches and mountains are the sum");
  await expect(block.getByRole("group", { name: FIRST })).toBeVisible();
  await expect(block.getByText("1 of 2")).toBeVisible();

  /** The block's height, and how far below its top the next text starts. */
  const measure = async () => {
    const box = await block.boundingBox();
    const below = await after.boundingBox();
    if (box === null || below === null) throw new Error("not laid out");
    return { height: box.height, below: below.y - box.y };
  };
  const first = await measure();

  await block.getByRole("button", { name: "Next question" }).click();
  await expect(block.getByRole("group", { name: SECOND })).toBeVisible();
  await expect(block.getByRole("group", { name: FIRST })).toHaveCount(0);
  await expect(block.getByText("2 of 2")).toBeVisible();

  expect(await measure()).toEqual(first);
});
