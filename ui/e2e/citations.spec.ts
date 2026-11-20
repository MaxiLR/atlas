import { expect, test, type Page } from "@playwright/test";

import { openApp, type AppOptions } from "./support/app.ts";

const TITLE = "Understanding plate motions, U.S. Geological Survey";

/** Opens Convergent boundaries, which cites two sources, and its first chip. */
async function openCitedSection(page: Page, options: AppOptions) {
  await openApp(page, options);
  await page.getByRole("button", { name: "Plate tectonics" }).click();
  const index = page.getByRole("navigation", { name: "Index" });
  await index.getByRole("button", { name: "Convergent boundaries" }).click();
  const article = page.getByRole("article", { name: "Convergent boundaries" });
  const chip = article
    .getByRole("button", { name: `Source 1: ${TITLE}` })
    .first();
  await expect(chip).toBeVisible();
  return chip;
}

const card = (page: Page) => page.getByRole("dialog", { name: "Source 1" });

for (const colorScheme of ["light", "dark"] as const) {
  test(`hovering and focusing a chip opens its card in the ${colorScheme} theme`, async ({
    page,
  }) => {
    const chip = await openCitedSection(page, { colorScheme });

    await chip.hover();
    await expect(card(page)).toBeVisible();
    await expect(card(page)).toContainText(TITLE);
    await expect(card(page)).toContainText("pubs.usgs.gov");

    await page.mouse.move(0, 0);
    await expect(card(page)).toBeHidden();

    await chip.focus();
    await expect(card(page)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(card(page)).toBeHidden();
    await expect(chip).toBeFocused();
  });
}

test("the card's control opens the source without navigating the window", async ({
  page,
}) => {
  const chip = await openCitedSection(page, {});
  const before = page.url();
  let opened = 0;
  page.context().on("page", () => (opened += 1));

  await chip.hover();
  await card(page)
    .getByRole("link", { name: /Open in browser/ })
    .click();

  expect(page.url()).toBe(before);
  expect(opened).toBe(0);
  await expect(chip).toBeVisible();
});

test("the card stays inside the window at 720 px", async ({ page }) => {
  const chip = await openCitedSection(page, { width: 720 });
  const viewport = page.viewportSize()!;

  await chip.hover();
  await expect(card(page)).toBeVisible();
  const box = (await card(page).boundingBox())!;
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
});
