import { expect, test, type Locator, type Page } from "@playwright/test";

import { openApp, type AppOptions } from "./support/app.ts";

const widths = [720, 1440] as const;
const schemes = ["light", "dark"] as const;

const PASSAGE =
  "Oceanic lithosphere is thinner but denser than continental lithosphere";

/** Opens Convergent boundaries of the sample course. */
async function openSection(page: Page, options: AppOptions) {
  await openApp(page, options);
  await page.getByRole("button", { name: "Plate tectonics" }).click();
  await page
    .getByRole("navigation", { name: "Index" })
    .getByRole("button", { name: "Convergent boundaries" })
    .click();
  const article = page.getByRole("article", { name: "Convergent boundaries" });
  await expect(article).toContainText(PASSAGE);
  return article;
}

/** Where `phrase` starts and ends on screen, on the middle of its line. */
async function phraseEnds(article: Locator, phrase: string) {
  return article.evaluate((root, wanted) => {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const at = node.nodeValue?.indexOf(wanted) ?? -1;
      if (at === -1) continue;
      const range = document.createRange();
      range.setStart(node, at);
      range.setEnd(node, at + wanted.length);
      const rects = [...range.getClientRects()];
      const first = rects[0]!;
      const last = rects.at(-1)!;
      return {
        start: { x: first.left, y: first.top + first.height / 2 },
        end: { x: last.right, y: last.top + last.height / 2 },
      };
    }
    throw new Error(`no text node with ${wanted}`);
  }, phrase);
}

/**
 * Selects `phrase`: by dragging the mouse across it, or with the keyboard:
 * Chromium extends a selection, not a caret, with Shift and the arrow keys
 * in text that is not editable, so its first word is taken and the arrow
 * keys extend that to the whole phrase.
 */
async function select(
  page: Page,
  article: Locator,
  phrase: string,
  by: "mouse" | "keyboard",
) {
  if (by === "mouse") {
    const { start, end } = await phraseEnds(article, phrase);
    await page.mouse.move(start.x + 1, start.y);
    await page.mouse.down();
    await page.mouse.move(end.x - 1, end.y, { steps: 8 });
    await page.mouse.up();
  } else {
    const word = phrase.split(" ")[0]!;
    const { start, end } = await phraseEnds(article, word);
    await page.mouse.dblclick((start.x + end.x) / 2, start.y);
    await expect
      .poll(() => page.evaluate(() => document.getSelection()?.toString()))
      .toBe(word);
    for (let step = word.length; step < phrase.length; step += 1) {
      await page.keyboard.press("Shift+ArrowRight");
    }
  }
  await expect
    .poll(() => page.evaluate(() => document.getSelection()?.toString() ?? ""))
    .toBe(phrase);
}

/** The element lies wholly inside the window. */
async function expectInWindow(page: Page, locator: Locator) {
  const box = await locator.boundingBox();
  if (box === null) throw new Error("not rendered");
  const size = page.viewportSize()!;
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(size.width);
  expect(box.y + box.height).toBeLessThanOrEqual(size.height);
}

const menu = (page: Page) => page.getByRole("toolbar", { name: "Selection" });
const definition = (page: Page, term: string) =>
  page.getByRole("dialog", { name: `Definition of ${term}` });

for (const width of widths) {
  for (const colorScheme of schemes) {
    test.describe(`a doubt at ${width} px, ${colorScheme}`, () => {
      test("a phrase selected with the mouse is defined, closed and reopened from its underline by keyboard", async ({
        page,
      }) => {
        const article = await openSection(page, { width, colorScheme });

        await select(page, article, "subduction", "mouse");
        await expect(menu(page)).toBeVisible();
        await expectInWindow(page, menu(page));
        await menu(page).getByRole("button", { name: "Define" }).click();

        const popover = definition(page, "subduction");
        await expect(popover).toBeVisible();
        await expect(popover).toContainText(
          "The sinking of one plate beneath another",
        );
        await expect(
          popover.getByRole("link", { name: "Open article" }),
        ).toBeVisible();
        await expect(popover).toContainText(
          "Subduction is a geological process",
        );
        await expectInWindow(page, popover);

        await page.keyboard.press("Escape");
        await expect(popover).toBeHidden();
        // Exact: a citation chip in the article is named "Source 2:
        // Subduction zones, …", which a substring match would find too.
        const term = article.getByRole("button", {
          name: "subduction",
          exact: true,
        });
        await expect(term).toBeFocused();

        await page.keyboard.press("Enter");
        await expect(popover).toBeVisible();
        await expect(popover).toContainText(
          "The sinking of one plate beneath another",
        );
        await expectInWindow(page, popover);
        await page.keyboard.press("Escape");
        await expect(popover).toBeHidden();
        await expect(term).toBeFocused();
      });

      test("a phrase selected with the keyboard is defined with D", async ({
        page,
      }) => {
        const article = await openSection(page, { width, colorScheme });

        await select(page, article, "fingernails grow", "keyboard");
        await expect(menu(page)).toBeVisible();
        await expectInWindow(page, menu(page));
        await page.keyboard.press("d");

        const popover = definition(page, "fingernails grow");
        await expect(popover).toBeVisible();
        await expect(popover).toContainText("No Wikipedia article matches");
        await expectInWindow(page, popover);

        await page.keyboard.press("Escape");
        await expect(popover).toBeHidden();
        await expect(
          article.getByRole("button", { name: "fingernails grow" }),
        ).toBeFocused();
      });

      test("asking about a passage opens the side panel on it, and its margin mark reopens it", async ({
        page,
      }) => {
        const article = await openSection(page, { width, colorScheme });

        await select(page, article, PASSAGE, "mouse");
        await menu(page).getByRole("button", { name: "Ask" }).click();

        const panel = page.getByRole("complementary", { name: "Side panel" });
        await expect(panel).toBeVisible();
        await expect(panel).toContainText(PASSAGE);
        const mark = article.getByRole("button", {
          name: /side chat on “Oceanic lithosphere/i,
        });
        await expect(mark).toHaveAttribute("aria-expanded", "true");

        await panel
          .getByRole("button", { name: "Close the side panel" })
          .click();
        await expect(panel).toBeHidden();
        await expect(mark).toBeFocused();
        await expect(mark).toHaveAttribute("aria-expanded", "false");

        await page.keyboard.press("Enter");
        await expect(panel).toBeVisible();
        await expect(panel).toContainText(PASSAGE);
        await expect(mark).toHaveAttribute("aria-expanded", "true");

        await page.keyboard.press("Escape");
        await expect(panel).toBeHidden();
        await expect(mark).toBeFocused();
      });
    });
  }
}

test("Escape closes the side panel at once after Ask, and after Define's popover closed", async ({
  page,
}) => {
  const article = await openSection(page, { width: 1440, colorScheme: "dark" });
  const panel = page.getByRole("complementary", { name: "Side panel" });

  await select(page, article, PASSAGE, "mouse");
  await menu(page).getByRole("button", { name: "Ask" }).click();
  await page.keyboard.press("Escape");
  await expect(panel).toBeHidden();

  await page.getByRole("button", { name: /side chat on “Oceanic/i }).focus();
  await page.keyboard.press("Enter");
  await expect(panel).toBeVisible();
  await select(page, article, "subduction", "mouse");
  await page.keyboard.press("d");
  const popover = definition(page, "subduction");
  await expect(popover).toContainText("Subduction is a geological process");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(popover).toBeHidden();
  await expect(panel).toBeHidden();
});
