import { expect, test, type Locator, type Page } from "@playwright/test";

import { openApp } from "./support/app.ts";

const widths = [720, 1024, 1440] as const;
const schemes = ["light", "dark"] as const;

async function box(locator: Locator) {
  const found = await locator.boundingBox();
  if (found === null) throw new Error("not rendered");
  return found;
}

/** Whether the page scrolls sideways. */
function scrollsSideways(page: Page) {
  return page.evaluate(
    () =>
      document.documentElement.scrollWidth >
      document.documentElement.clientWidth,
  );
}

/** Opens Plate tectonics on Convergent boundaries; returns the regions. */
async function openReader(page: Page) {
  await page.getByRole("button", { name: "Plate tectonics" }).click();
  const index = page.getByRole("navigation", { name: "Index" });
  await index.getByRole("button", { name: "Convergent boundaries" }).click();
  await expect(
    page.getByRole("article", { name: "Convergent boundaries" }),
  ).toBeVisible();
  return {
    rail: page.getByRole("complementary", { name: "Course" }),
    index,
    column: page.getByRole("main"),
    panel: page.getByRole("complementary", { name: "Side panel" }),
  };
}

for (const width of widths) {
  for (const scheme of schemes) {
    test.describe(`the reader frame at ${width} px, ${scheme}`, () => {
      test.beforeEach(async ({ page }) => {
        await openApp(page, { width, colorScheme: scheme });
      });

      test("the library and the reader do not scroll sideways", async ({
        page,
      }) => {
        await expect(
          page.getByRole("textbox", { name: "Topic" }),
        ).toBeVisible();
        expect(await scrollsSideways(page)).toBe(false);

        await openReader(page);
        expect(await scrollsSideways(page)).toBe(false);
        await page.getByRole("button", { name: /^Side chats/ }).click();
        await expect(
          page.getByRole("complementary", { name: "Side panel" }),
        ).toBeVisible();
        expect(await scrollsSideways(page)).toBe(false);
      });

      if (width < 1024) {
        test("the rail folds to 72 px and the index button opens the titles over the column", async ({
          page,
        }) => {
          const { rail, index, column } = await openReader(page);
          expect((await box(rail)).width).toBe(72);
          const title = index.getByText("Transform boundaries");
          await expect(title).toBeHidden();
          const columnBefore = await box(column);

          const toggle = page.getByRole("button", { name: "Show the index" });
          await toggle.click();
          await expect(toggle).toHaveAttribute("aria-expanded", "true");
          await expect(title).toBeVisible();
          expect((await box(index)).width).toBeGreaterThan(200);
          expect(await box(column)).toEqual(columnBefore);

          await page.getByRole("button", { name: "Hide the index" }).click();
          await expect(title).toBeHidden();
          await expect(toggle).toHaveAttribute("aria-expanded", "false");

          await toggle.focus();
          await page.keyboard.press("Enter");
          await expect(title).toBeVisible();
          await expect(
            index.getByRole("button", { name: "Convergent boundaries" }),
          ).toBeFocused();
          await page.keyboard.press("Escape");
          await expect(title).toBeHidden();
          await expect(toggle).toBeFocused();
        });

        test("the side panel opens over the reading column, at most 420 px wide", async ({
          page,
        }) => {
          const { column, panel } = await openReader(page);
          const columnBefore = await box(column);

          await page.getByRole("button", { name: /^Side chats/ }).click();

          await expect(panel).toBeVisible();
          const over = await box(panel);
          expect(over.width).toBeLessThanOrEqual(420);
          expect(over.x).toBeLessThan(columnBefore.x + columnBefore.width);
          expect(await box(column)).toEqual(columnBefore);
        });
      } else {
        test("rail, column and side panel sit side by side, and the text keeps its measure", async ({
          page,
        }) => {
          const { rail, column, panel } = await openReader(page);
          await expect(
            page.getByRole("button", { name: "Show the index" }),
          ).toBeHidden();
          await expect(
            page
              .getByRole("navigation", { name: "Index" })
              .getByText("Transform boundaries"),
          ).toBeVisible();

          await page.getByRole("button", { name: /^Side chats/ }).click();
          await expect(panel).toBeVisible();

          const [left, middle, right] = [
            await box(rail),
            await box(column),
            await box(panel),
          ];
          expect(left.width).toBe(320);
          expect(right.width).toBe(400);
          expect(left.x + left.width).toBeLessThanOrEqual(middle.x);
          expect(middle.x + middle.width).toBeLessThanOrEqual(right.x);
          expect(right.x + right.width).toBeLessThanOrEqual(width);

          const article = page.getByRole("article", {
            name: "Convergent boundaries",
          });
          const measure = article.locator("p").first();
          expect((await box(measure)).width).toBeLessThanOrEqual(680);
          await page
            .getByRole("button", { name: "Close the side panel" })
            .click();
          expect((await box(measure)).width).toBeLessThanOrEqual(680);
        });
      }
    });
  }
}
