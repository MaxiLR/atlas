import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { openApp } from "./support/app.ts";

const schemes = ["light", "dark"] as const;

const longTopic =
  "The history and practice of cartography, from Ptolemy's Geography and the portolan charts to satellite geodesy and the projections behind modern web maps";

/** The row of the course on `topic` in the Courses list. */
function row(page: Page, topic: string): Locator {
  return page
    .getByRole("region", { name: "Courses" })
    .getByRole("listitem")
    .filter({
      has: page.getByRole("button", { name: topic, exact: true }),
    });
}

async function box(locator: Locator) {
  const found = await locator.boundingBox();
  if (found === null) throw new Error("not laid out");
  return found;
}

test("with no courses only the topic field shows", async ({ page }) => {
  await openApp(page, { scenario: "empty-library" });

  await expect(page.getByRole("textbox", { name: "Topic" })).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "What do you want to learn?" }),
  ).toBeVisible();
  await expect(page.getByText("Courses are written in English.")).toBeVisible();
  await expect(page.getByRole("region", { name: "Courses" })).toHaveCount(0);
  await expect(page.getByRole("list")).toHaveCount(0);
});

test("a course starts from the keyboard alone", async ({ page }) => {
  await openApp(page, { scenario: "empty-library" });
  const field = page.getByRole("textbox", { name: "Topic" });
  await expect(field).toBeVisible();

  for (let presses = 0; presses < 5; presses += 1) {
    if (await field.evaluate((element) => element === document.activeElement))
      break;
    await page.keyboard.press("Tab");
  }
  await expect(field).toBeFocused();
  await page.keyboard.type("Volcanoes");
  await page.keyboard.press("Enter");

  await expect(
    page.getByRole("group", { name: "How well do you know it?" }),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Volcanoes" })).toBeVisible();
});

test("opening a course lands on the section it resumes at", async ({
  page,
}) => {
  await openApp(page);
  const plate = row(page, "Plate tectonics");
  await expect(
    plate.getByRole("button", { name: "Plate tectonics", exact: true }),
  ).toHaveAccessibleDescription("Resumes at Convergent boundaries");

  await plate
    .getByRole("button", { name: "Plate tectonics", exact: true })
    .click();

  await expect(
    page.getByRole("article", { name: "Convergent boundaries" }),
  ).toBeVisible();
});

test("a course is deleted through the confirmation and leaves the list", async ({
  page,
}) => {
  await openApp(page);
  const plate = row(page, "Plate tectonics");

  await plate.getByRole("button", { name: "Course menu" }).click();
  await page.getByRole("menuitem", { name: "Delete course" }).click();
  const dialog = page.getByRole("alertdialog", {
    name: "Delete Plate tectonics?",
  });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("permanent");
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeFocused();
  await dialog.getByRole("button", { name: "Delete course" }).click();

  await expect(dialog).toHaveCount(0);
  await expect(plate).toHaveCount(0);
  await expect(row(page, "The French Revolution")).toBeVisible();
});

for (const scheme of schemes) {
  test.describe(`the library, ${scheme}`, () => {
    test("at 1440 px a row keeps topic, progress and date on one line", async ({
      page,
    }) => {
      await openApp(page, {
        scenario: "library",
        width: 1440,
        colorScheme: scheme,
      });
      const plate = row(page, "Plate tectonics");
      const topic = await box(
        plate.getByRole("button", { name: "Plate tectonics", exact: true }),
      );
      const track = await box(plate.getByRole("progressbar"));
      const date = await box(plate.locator("time"));

      expect(track.x).toBeGreaterThan(topic.x);
      expect(track.y).toBeLessThan(topic.y + topic.height);
      expect(date.x).toBeGreaterThan(track.x + track.width);
      expect(date.y).toBeLessThan(topic.y + topic.height);
    });

    test("at 720 px rows stack: progress and date under the topic", async ({
      page,
    }) => {
      await openApp(page, {
        scenario: "library",
        width: 720,
        colorScheme: scheme,
      });
      for (const topic of [
        "Plate tectonics",
        "Bayesian inference",
        longTopic,
      ]) {
        const course = row(page, topic);
        const open = await box(course.getByRole("button").first());
        const track = await box(course.getByRole("progressbar"));
        const date = await box(course.locator("time"));
        const menu = await box(
          course.getByRole("button", { name: "Course menu" }),
        );

        expect(track.y).toBeGreaterThanOrEqual(open.y + open.height - 1);
        expect(date.y).toBeGreaterThanOrEqual(open.y + open.height - 1);
        expect(date.x).toBeGreaterThan(track.x + track.width);
        expect(menu.x).toBeGreaterThan(open.x + open.width - 1);
        expect(menu.y).toBeLessThan(open.y + open.height);
      }
      expect(
        await page.evaluate(
          () =>
            document.documentElement.scrollWidth >
            document.documentElement.clientWidth,
        ),
      ).toBe(false);
    });

    test("many courses scroll under the window bar", async ({ page }) => {
      await openApp(page, {
        scenario: "library",
        width: 720,
        colorScheme: scheme,
      });
      const last = row(page, "Japanese woodblock printing");
      await expect(last).toBeAttached();
      await expect(last).not.toBeInViewport();

      await last.scrollIntoViewIfNeeded();

      await expect(last).toBeInViewport();
      await expect(
        page.getByRole("button", { name: "Settings" }),
      ).toBeInViewport();
    });

    test("has no detectable accessibility violations, with the deletion asked", async ({
      page,
    }) => {
      await openApp(page, {
        scenario: "library",
        width: 1024,
        colorScheme: scheme,
      });
      await expect(row(page, "Plate tectonics")).toBeVisible();
      const scan = () =>
        new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa"]).analyze();

      expect((await scan()).violations).toEqual([]);

      await row(page, "Plate tectonics")
        .getByRole("button", { name: "Course menu" })
        .click();
      await page.getByRole("menuitem", { name: "Delete course" }).click();
      await expect(page.getByRole("alertdialog")).toBeVisible();
      expect((await scan()).violations).toEqual([]);
    });
  });
}
