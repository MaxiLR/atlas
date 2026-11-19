import { expect, test } from "@playwright/test";

import { openApp } from "./support/app.ts";

const STEM =
  "A plate carrying young, warm ocean floor meets a plate carrying old, cold ocean floor. What happens next?";
const EXPLANATION =
  "Ocean floor grows colder and denser as it ages, so the old plate is the heavier one and sinks beneath the younger, more buoyant plate.";

for (const colorScheme of ["light", "dark"] as const) {
  test(`a check question answered by keyboard shows its result and stays answered in the ${colorScheme} theme`, async ({
    page,
  }) => {
    await openApp(page, { colorScheme });
    await page.getByRole("button", { name: "Plate tectonics" }).click();
    const index = page.getByRole("navigation", { name: "Index" });
    await index.getByRole("button", { name: "Convergent boundaries" }).click();
    const article = page.getByRole("article", {
      name: "Convergent boundaries",
    });
    const question = article.getByRole("group", { name: STEM });
    const radios = question.getByRole("radio");
    await expect(radios).toHaveCount(4);
    const check = article.getByRole("button", { name: "Check answer" });
    await expect(check).toBeDisabled();

    await radios.first().focus();
    await page.keyboard.press("ArrowDown");
    await expect(radios.nth(1)).toBeFocused();
    await expect(radios.nth(1)).toBeChecked();
    await page.keyboard.press("Tab");
    await expect(check).toBeFocused();
    await page.keyboard.press("Enter");

    await expect(article.getByText(EXPLANATION)).toBeVisible();
    await expect(
      article.getByRole("status").getByText("Incorrect"),
    ).toBeVisible();
    await expect(question.locator("label").nth(1)).toContainText(
      "Your answer, incorrect",
    );
    await expect(question.locator("label").nth(0)).toContainText(
      "Correct answer",
    );
    for (const radio of await radios.all()) await expect(radio).toBeDisabled();

    await index.getByRole("button", { name: "Divergent boundaries" }).click();
    await expect(
      page.getByRole("article", { name: "Divergent boundaries" }),
    ).toBeVisible();
    await index.getByRole("button", { name: "Convergent boundaries" }).click();

    await expect(article.getByText(EXPLANATION)).toBeVisible();
    await expect(radios.nth(1)).toBeChecked();
    await expect(question.locator("label").nth(1)).toContainText(
      "Your answer, incorrect",
    );
    for (const radio of await radios.all()) await expect(radio).toBeDisabled();
    await expect(
      article.getByRole("button", { name: "Check answer" }),
    ).toHaveCount(0);
  });
}
