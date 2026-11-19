import { expect, test } from "@playwright/test";

test("topic, index, a streamed section, then Next", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("textbox", { name: "Topic" }).fill("Plate tectonics");
  await page.getByRole("button", { name: "Start course" }).click();

  const index = page.getByRole("navigation", { name: "Index" });
  await expect(
    page.getByRole("heading", { name: "Plate tectonics" }),
  ).toBeVisible();
  await expect(index.getByRole("button")).toHaveCount(6);

  await index.getByRole("button", { name: "What it is" }).click();
  const first = page.getByRole("article", { name: "What it is" });
  await expect(first).toContainText("This section opens with the main idea.");
  await expect(first).toContainText("It closes with what comes next.");

  await page.getByRole("button", { name: "Next" }).click();
  const second = page.getByRole("article", { name: "Why it matters" });
  await expect(second).toContainText("It closes with what comes next.");
  await expect(first).toHaveCount(0);
});
