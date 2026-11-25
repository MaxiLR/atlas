import { expect, test } from "@playwright/test";

import { openApp } from "./support/app.ts";

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
