import { expect, test, type Page } from "@playwright/test";

import { openApp } from "./support/app.ts";

const widths = [720, 1024, 1440] as const;

/** How far the page itself could scroll; the window is never a page. */
function pageRoom(page: Page) {
  return page.evaluate(() => {
    const root = document.documentElement;
    return {
      across: root.scrollWidth - root.clientWidth,
      down: root.scrollHeight - root.clientHeight,
    };
  });
}

// The webview rubber-bands its root scroller under a trackpad swipe unless
// the root says otherwise, which drags the whole interface about. Chromium
// has no rubber band to watch here, so this reads what the root declares.
test("a trackpad swipe past the window's edge does not drag the interface", async ({
  page,
}) => {
  await openApp(page);
  await expect(
    page.getByRole("button", { name: "Plate tectonics" }),
  ).toBeVisible();

  const overscroll = await page.evaluate(() => {
    const root = getComputedStyle(document.documentElement);
    return { x: root.overscrollBehaviorX, y: root.overscrollBehaviorY };
  });

  expect(overscroll).toEqual({ x: "none", y: "none" });
});

for (const width of widths) {
  test(`at ${width} px the reader scrolls only inside its own regions`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 500 });
    await openApp(page);
    await page.getByRole("button", { name: "Plate tectonics" }).click();
    await page
      .getByRole("navigation", { name: "Index" })
      .getByRole("button", { name: "Convergent boundaries" })
      .click();
    // The section ends with its sources, far below a 500 px window.
    await expect(
      page.getByRole("article", { name: "Convergent boundaries" }),
    ).toBeVisible();
    await expect(
      page.getByRole("link", { name: /opens in your browser/ }).first(),
    ).toBeAttached();
    expect(await pageRoom(page)).toEqual({ across: 0, down: 0 });
    // A swipe over the rail, and one past the column's end, move nothing.
    const rail = page.getByRole("complementary", { name: "Course" });
    await rail.hover();
    await page.mouse.wheel(0, 400);
    await page.getByRole("main").hover();
    await page.mouse.wheel(0, 5000);
    await page.mouse.wheel(400, 0);
    await expect
      .poll(() => page.evaluate(() => [window.scrollX, window.scrollY]))
      .toEqual([0, 0]);

    await page.getByRole("button", { name: /^Side chats/ }).click();
    const panel = page.getByRole("complementary", { name: "Side panel" });
    await expect(panel).toBeVisible();
    expect(await pageRoom(page)).toEqual({ across: 0, down: 0 });
    await panel.getByRole("tab", { name: /^Glossary/ }).click();
    expect(await pageRoom(page)).toEqual({ across: 0, down: 0 });
  });
}
