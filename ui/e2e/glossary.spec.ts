import { expect, test, type Locator } from "@playwright/test";

import { openApp } from "./support/app.ts";

const widths = [720, 1440] as const;
const schemes = ["light", "dark"] as const;

async function box(locator: Locator) {
  const found = await locator.boundingBox();
  if (found === null) throw new Error("not rendered");
  return found;
}

for (const width of widths) {
  for (const scheme of schemes) {
    test.describe(`the glossary at ${width} px, ${scheme}`, () => {
      test.beforeEach(async ({ page }) => {
        await openApp(page, { width, colorScheme: scheme });
      });

      test("opens from the header button, lists the course's entries and opens an entry's section", async ({
        page,
      }) => {
        await page.getByRole("button", { name: "Plate tectonics" }).click();
        await page
          .getByRole("navigation", { name: "Index" })
          .getByRole("button", { name: "Divergent boundaries" })
          .click();
        await expect(
          page.getByRole("article", { name: "Divergent boundaries" }),
        ).toBeVisible();
        const column = page.getByRole("main");
        const columnBefore = await box(column);

        const opener = page.getByRole("button", { name: "Glossary 1" });
        await opener.click();

        const panel = page.getByRole("complementary", { name: "Side panel" });
        await expect(panel).toBeVisible();
        await expect(
          panel.getByRole("tab", { name: "Glossary 1" }),
        ).toHaveAttribute("aria-selected", "true");
        const entries = panel
          .getByRole("list", { name: "Glossary" })
          .getByRole("listitem");
        await expect(entries).toHaveCount(1);
        const [entry] = await entries.all();
        await expect(entry!.getByRole("term")).toHaveText("asthenosphere");
        await expect(entry!.getByRole("definition")).toContainText(
          "The layer of the mantle directly beneath the plates.",
        );

        if (width < 1024) {
          const over = await box(panel);
          expect(over.width).toBeLessThanOrEqual(420);
          expect(over.x).toBeLessThan(columnBefore.x + columnBefore.width);
          expect(await box(column)).toEqual(columnBefore);
        }

        const source = entry!.getByRole("button", {
          name: "Convergent boundaries",
        });
        await source.click();
        await expect(
          page.getByRole("article", { name: "Convergent boundaries" }),
        ).toBeAttached();
        await expect(panel).toBeVisible();

        if (width < 1024) {
          await source.focus();
          await page.keyboard.press("Escape");
          await expect(panel).toBeHidden();
          await expect(opener).toBeFocused();
          await expect(
            page.getByRole("article", { name: "Convergent boundaries" }),
          ).toBeVisible();
        }
      });

      test("links an entry whose article was found, and opening it leaves the app where it was", async ({
        page,
      }) => {
        await page.getByRole("button", { name: "Plate tectonics" }).click();
        await page
          .getByRole("navigation", { name: "Index" })
          .getByRole("button", { name: "Convergent boundaries" })
          .click();
        const article = page.getByRole("article", {
          name: "Convergent boundaries",
        });
        await article
          .getByRole("button", { name: "asthenosphere", exact: true })
          .click();
        const popover = page.getByRole("dialog", {
          name: "Definition of asthenosphere",
        });
        await expect(popover).toContainText(
          "The asthenosphere is the mechanically weak",
        );
        await page.keyboard.press("Escape");
        await expect(popover).toBeHidden();

        await page.getByRole("button", { name: "Glossary 1" }).click();
        const panel = page.getByRole("complementary", { name: "Side panel" });
        const entry = panel
          .getByRole("list", { name: "Glossary" })
          .getByRole("listitem");
        const link = entry.getByRole("link", {
          name: "Wikipedia article on asthenosphere",
        });
        await expect(link).toBeVisible();
        await expect(link).toHaveText("Wikipedia");
        await expect(link).toHaveAttribute(
          "href",
          "https://en.wikipedia.org/wiki/Asthenosphere",
        );
        await expect(
          entry.getByRole("button", { name: "Convergent boundaries" }),
        ).toBeVisible();

        const before = page.url();
        await link.click();
        expect(page.url()).toBe(before);
        await expect(panel).toBeVisible();
        await expect(article).toBeAttached();
      });
    });
  }
}
