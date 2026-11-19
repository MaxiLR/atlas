import { expect, test, type Page } from "@playwright/test";

import { openApp } from "../support/app.ts";

/** One step of the fake core's pace in Vite mode `fake`. */
const STEP = 80;

const QUESTION = "Why do plates move at all?";
const FIRST_CHUNK =
  "For plates with a subducting edge, slab pull is the larger force.";
const LAST_CHUNK = "the cold, downward limb of the mantle's circulation.";
const FOLLOW_UP = "What is ridge push, and how much does it add?";
const SOURCE_TITLE =
  "On the relative importance of the driving forces of plate motion, Forsyth and Uyeda";

/** Holds the page's clock, so the fake core takes a step only on `runFor`. */
async function holdClock(page: Page) {
  const now = await page.evaluate(() => Date.now());
  await page.clock.pauseAt(now + 1_000);
}

test("a side chat started from the panel: a question by keyboard, a streamed reply with citations, a stopped follow-up, and the list with it first", async ({
  page,
}, testInfo) => {
  const { viewport, colorScheme } = testInfo.project.use;
  const width = viewport?.width ?? 1440;
  await page.clock.install();
  await openApp(page, { width, colorScheme: colorScheme ?? undefined });
  await page.getByRole("button", { name: "Plate tectonics" }).click();
  await page
    .getByRole("navigation", { name: "Index" })
    .getByRole("button", { name: "Convergent boundaries" })
    .click();
  const article = page.getByRole("article", { name: "Convergent boundaries" });
  await expect(article).toContainText("slab pull");

  await page.getByRole("button", { name: "Side chats 1" }).click();
  const panel = page.getByRole("complementary", { name: "Side panel" });
  await expect(panel.getByRole("list", { name: "Side chats" })).toBeVisible();

  // Below 1024 px the panel lies over the reading column; wider, beside it.
  const panelBox = (await panel.boundingBox())!;
  const columnBox = (await article.boundingBox())!;
  if (width < 1024) {
    expect(panelBox.x).toBeLessThan(columnBox.x + columnBox.width);
  } else {
    expect(panelBox.x).toBeGreaterThanOrEqual(columnBox.x + columnBox.width);
  }

  await panel.getByRole("button", { name: "New side chat" }).click();
  const thread = panel.getByRole("region", { name: "Side chat" });
  await expect(thread.getByRole("blockquote")).toHaveCount(0);
  const field = panel.getByRole("textbox", { name: "Ask a question" });
  await expect(field).toBeFocused();

  // The reply streams a step at a time.
  await page.keyboard.type(QUESTION);
  await holdClock(page);
  await page.keyboard.press("Enter");
  await expect(thread.getByText(QUESTION)).toBeVisible();
  await expect(thread.getByText("Searching the web…")).toBeVisible();
  await expect(panel.getByRole("button", { name: "Stop" })).toBeVisible();
  await page.clock.runFor(2 * STEP);
  await expect(thread).toContainText(FIRST_CHUNK);
  await expect(thread).not.toContainText(LAST_CHUNK);
  await page.clock.resume();
  await expect(thread).toContainText(LAST_CHUNK);
  await expect(thread.getByText("Searched the web")).toBeVisible();

  // A citation opens its card on hover.
  await thread
    .getByRole("button", { name: `Source 1: ${SOURCE_TITLE}` })
    .hover();
  const card = page.getByRole("dialog", { name: "Source 1" });
  await expect(card).toBeVisible();
  await expect(card).toContainText(SOURCE_TITLE);
  await page.mouse.move(0, 0);
  await expect(card).toBeHidden();

  // A follow-up sends itself; its reply is stopped part way.
  const followUp = thread.getByRole("button", { name: FOLLOW_UP });
  await expect(followUp).toBeVisible();
  await holdClock(page);
  await followUp.click();
  await expect(thread.getByText(FOLLOW_UP)).toBeVisible();
  await page.clock.runFor(2 * STEP);
  await expect(thread.getByText(FIRST_CHUNK, { exact: false })).toHaveCount(2);
  await panel.getByRole("button", { name: "Stop" }).click();
  await page.clock.runFor(STEP);
  await expect(
    thread.getByText("You stopped this reply before it was finished."),
  ).toBeVisible();
  await expect(panel.getByRole("button", { name: "Stop" })).toHaveCount(0);
  await page.clock.resume();

  // Back to the list, where this chat comes first.
  await panel.getByRole("button", { name: "All side chats" }).click();
  const entries = panel
    .getByRole("list", { name: "Side chats" })
    .getByRole("listitem");
  await expect(entries).toHaveCount(2);
  await expect(entries.first()).toContainText(QUESTION);
});
