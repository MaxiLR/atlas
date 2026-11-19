import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { openApp } from "./support/app.ts";

const schemes = ["light", "dark"] as const;

/** The questions the fake core asks for a new course, in order. */
const stems = [
  "An oceanic plate and a continental plate are moving toward each other. What happens where they meet?",
  "Two plates pull apart in the middle of an ocean. What forms between them?",
  "A slab of old ocean floor sinks into the mantle. What happens to the plate it is attached to?",
  "You heat a pot of thick soup from below. What does the soup do, and what does it suggest about the mantle?",
  "Two plates slide past each other sideways, and their edges catch. What happens when the strain is released?",
  "A string of volcanic islands grows older the further it lies from one active volcano. What does the pattern show?",
];

async function box(locator: Locator) {
  const found = await locator.boundingBox();
  if (found === null) throw new Error("not laid out");
  return found;
}

/** Presses Tab until `target` has focus, at most `limit` times. */
async function tabTo(page: Page, target: Locator, limit = 8) {
  for (let presses = 0; presses < limit; presses += 1) {
    if (await target.evaluate((element) => element === document.activeElement))
      return;
    await page.keyboard.press("Tab");
  }
  await expect(target).toBeFocused();
}

/** Starts a course on `topic` from the library by mouse. */
async function startCourse(page: Page, topic = "Plate tectonics") {
  await page.getByRole("textbox", { name: "Topic" }).fill(topic);
  await page.getByRole("button", { name: "Start course" }).click();
}

/** Chooses a self-rating and a goal by mouse, then continues. */
async function rate(page: Page) {
  await page.getByRole("radio", { name: "Know the basics" }).check();
  await page.getByRole("radio", { name: "Working knowledge" }).check();
  await page.getByRole("button", { name: "Continue" }).click();
}

/** Answers the question shown by its key, then Enter. */
async function answerByKey(page: Page, number: number, key: string) {
  const group = page.getByRole("group", { name: stems[number - 1] });
  await expect(group).toBeFocused();
  await expect(page.getByText(`Question ${number} of 4 to 6`)).toBeVisible();
  await page.keyboard.press(key);
  await page.keyboard.press("Enter");
}

test("a full placement by keyboard alone: the rating and goal, four questions, then the index opens on its first section", async ({
  page,
}) => {
  await openApp(page, { scenario: "empty-library" });
  const field = page.getByRole("textbox", { name: "Topic" });
  await tabTo(page, field);
  await page.keyboard.type("Plate tectonics");
  await page.keyboard.press("Enter");

  const ratings = page.getByRole("group", { name: "How well do you know it?" });
  await expect(ratings).toBeVisible();
  await tabTo(page, ratings.getByRole("radio").first());
  await page.keyboard.press("ArrowDown");
  await expect(
    ratings.getByRole("radio", { name: "Know the basics" }),
  ).toBeChecked();
  const goals = page.getByRole("group", { name: "What is your goal?" });
  await tabTo(page, goals.getByRole("radio").first());
  await page.keyboard.press("Space");
  await expect(goals.getByRole("radio", { name: "Overview" })).toBeChecked();
  await tabTo(page, page.getByRole("button", { name: "Continue" }));
  await page.keyboard.press("Enter");

  for (const [index, key] of ["1", "0", "3", "2"].entries()) {
    await answerByKey(page, index + 1, key);
  }

  await expect(
    page.getByRole("article", { name: "Layers by composition" }),
  ).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Index" })).toBeVisible();
});

test("a placement that asks six questions counts on to six, then builds the index", async ({
  page,
}) => {
  await openApp(page, { scenario: "placement-six-questions" });
  await startCourse(page);
  await rate(page);

  const steps = page.getByRole("list", { name: "Questions" });
  for (let number = 1; number <= 6; number += 1) {
    await answerByKey(page, number, String(((number - 1) % 4) + 1));
    if (number === 4) {
      await expect(page.getByText("Question 5 of 4 to 6")).toBeVisible();
      await expect(steps.getByRole("listitem")).toHaveText([
        "Question 1, answered",
        "Question 2, answered",
        "Question 3, answered",
        "Question 4, answered",
        "Question 5, current",
        "Question 6, may not be asked",
      ]);
    }
  }

  await expect(
    page.getByRole("article", { name: "Layers by composition" }),
  ).toBeVisible();
});

test("a failed step says what failed, and Retry builds the index", async ({
  page,
}) => {
  await openApp(page, { scenario: "placement-step-fails" });
  await startCourse(page);
  await rate(page);
  for (let number = 1; number <= 4; number += 1) {
    await answerByKey(page, number, "0");
  }

  await expect(page.getByRole("alert")).toHaveText(
    "The index could not be built. The request failed after 4 attempts.",
  );
  await page.getByRole("button", { name: "Retry" }).click();

  await expect(
    page.getByRole("article", { name: "Layers by composition" }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("leaving mid-placement keeps the course, and opening it again resumes at the next question", async ({
  page,
}) => {
  await openApp(page, { scenario: "empty-library" });
  await startCourse(page);
  await rate(page);
  await answerByKey(page, 1, "2");
  await expect(page.getByRole("group", { name: stems[1] })).toBeFocused();

  await page.getByRole("button", { name: "Library" }).click();
  const course = page.getByRole("button", {
    name: "Plate tectonics",
    exact: true,
  });
  await expect(course).toBeVisible();
  await course.click();

  await expect(page.getByText("Question 2 of 4 to 6")).toBeVisible();
  await answerByKey(page, 2, "4");
  await answerByKey(page, 3, "1");
  await answerByKey(page, 4, "1");
  await expect(
    page.getByRole("article", { name: "Layers by composition" }),
  ).toBeVisible();
});

for (const scheme of schemes) {
  test.describe(`placement, ${scheme}`, () => {
    test("at 1440 px the note sits beside Continue", async ({ page }) => {
      await openApp(page, {
        scenario: "default",
        width: 1440,
        colorScheme: scheme,
      });
      await page.getByRole("button", { name: "The French Revolution" }).click();
      const note = page.getByText("Nothing is marked right or wrong here.");
      const next = page.getByRole("button", { name: "Continue" });
      await expect(next).toBeVisible();

      const noteBox = await box(note);
      const nextBox = await box(next);
      expect(nextBox.x).toBeGreaterThan(noteBox.x + noteBox.width);
      expect(nextBox.y).toBeLessThan(noteBox.y + noteBox.height);
    });

    test("at 720 px the actions stack: Continue full width above the note", async ({
      page,
    }) => {
      await openApp(page, {
        scenario: "default",
        width: 720,
        colorScheme: scheme,
      });
      await page.getByRole("button", { name: "The French Revolution" }).click();
      await expect(page.getByText("Question 2 of 4 to 6")).toBeVisible();
      const group = page.getByRole("group", {
        name: "Paris crowds take the Bastille in July 1789. What happens in the countryside in the weeks after?",
      });
      const note = page.getByText("Nothing is marked right or wrong here.");
      const next = page.getByRole("button", { name: "Continue" });
      await page.keyboard.press("3");

      const groupBox = await box(group);
      const noteBox = await box(note);
      const nextBox = await box(next);
      expect(nextBox.y + nextBox.height).toBeLessThanOrEqual(noteBox.y + 1);
      expect(Math.abs(nextBox.width - groupBox.width)).toBeLessThanOrEqual(1);
      expect(
        await page.evaluate(
          () =>
            document.documentElement.scrollWidth >
            document.documentElement.clientWidth,
        ),
      ).toBe(false);
    });

    test("has no detectable accessibility violations on the rating, a question, a waiting state and a failed step", async ({
      page,
    }) => {
      await openApp(page, {
        scenario: "placement-step-fails",
        width: 1024,
        colorScheme: scheme,
      });
      const scan = async () =>
        (
          await new AxeBuilder({ page })
            .withTags(["wcag2a", "wcag2aa"])
            .analyze()
        ).violations;

      await startCourse(page);
      await expect(
        page.getByRole("group", { name: "How well do you know it?" }),
      ).toBeVisible();
      expect(await scan()).toEqual([]);

      await rate(page);
      await expect(page.getByRole("group", { name: stems[0] })).toBeFocused();
      await page.keyboard.press("2");
      expect(await scan()).toEqual([]);
      await page.keyboard.press("Enter");

      for (let number = 2; number <= 4; number += 1) {
        await answerByKey(page, number, "1");
      }
      await expect(page.getByRole("alert")).toBeVisible();
      expect(await scan()).toEqual([]);
    });
  });
}
