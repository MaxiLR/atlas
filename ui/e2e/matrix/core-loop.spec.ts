import { expect, test } from "@playwright/test";

import { openApp } from "../support/app.ts";

test("first launch, sign in, topic, placement, index, a streamed section, then Next", async ({
  page,
}, testInfo) => {
  const { viewport, colorScheme } = testInfo.project.use;
  await openApp(page, {
    scenario: "first-launch",
    width: viewport?.width,
    colorScheme: colorScheme ?? undefined,
  });

  await page.getByRole("button", { name: "Continue with ChatGPT" }).click();

  await page.getByRole("textbox", { name: "Topic" }).fill("Plate tectonics");
  await page.getByRole("button", { name: "Start course" }).click();

  await expect(
    page.getByRole("heading", { name: "Plate tectonics" }),
  ).toBeVisible();
  await page
    .getByRole("group", { name: "How well do you know it?" })
    .getByRole("radio", { name: "Know the basics" })
    .check();
  await page
    .getByRole("group", { name: "What is your goal?" })
    .getByRole("radio", { name: "Working knowledge" })
    .check();
  await page.getByRole("button", { name: "Continue" }).click();

  for (let number = 1; number <= 4; number += 1) {
    await expect(page.getByText(`Question ${number} of 4 to 6`)).toBeVisible();
    const answer =
      number === 3
        ? page.getByRole("radio", { name: "Not sure" })
        : page.getByRole("radio").first();
    await answer.check();
    await page.getByRole("button", { name: "Continue" }).click();
  }

  const index = page.getByRole("navigation", { name: "Index" });
  // Three chapter rows and the five sections of the first two chapters; the
  // chapter further along is a station with its section count.
  await expect(index.getByRole("button")).toHaveCount(8);
  for (const chapter of [
    "The solid Earth",
    "Plates and their boundaries",
    "What moves the plates",
  ]) {
    await expect(
      index.getByRole("button", { name: chapter, exact: true }),
    ).toHaveAccessibleDescription(/^Chapter, \d+ sections$/);
  }
  for (const section of [
    "Layers by composition",
    "Lithosphere and asthenosphere",
    "Divergent boundaries",
    "Convergent boundaries",
    "Transform boundaries",
  ]) {
    await expect(
      index.getByRole("button", { name: section, exact: true }),
    ).toBeVisible();
  }
  await expect(
    index.getByRole("button", { name: "What moves the plates" }),
  ).toHaveAccessibleDescription("Chapter, 2 sections");
  await expect(
    index.getByRole("button", { name: "Mantle convection" }),
  ).toHaveCount(0);

  await index.getByRole("button", { name: "Layers by composition" }).click();
  const first = page.getByRole("article", { name: "Layers by composition" });
  await expect(first).toContainText(
    "Earth is layered by what its rock is made of.",
  );
  await expect(first).toContainText("roughly what the skin is to an apple.");

  await page.getByRole("button", { name: "Next" }).click();
  const second = page.getByRole("article", {
    name: "Lithosphere and asthenosphere",
  });
  await expect(second).toContainText("and the plates ride on it.");
  await expect(first).toHaveCount(0);
});
