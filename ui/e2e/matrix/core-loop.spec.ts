import { expect, test } from "@playwright/test";

import { openApp } from "../support/app";

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
  await expect(index.getByRole("button")).toHaveCount(7);

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
