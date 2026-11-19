import { expect, test, type Page } from "@playwright/test";
import { differenceEuclidean, wcagContrast } from "culori";

import { openApp } from "./support/app.ts";

const distance = differenceEuclidean("oklab");

/** The light theme's reading ground. */
const LIGHT_CANVAS = "#fdfdfd";

/** Every entry of Plate tectonics' index, in index order. */
const contents = [
  "The solid Earth",
  "Layers by composition",
  "Lithosphere and asthenosphere",
  "Plates and their boundaries",
  "Divergent boundaries",
  "Convergent boundaries",
  "Subduction, step by step",
  "Why the denser plate sinks",
  "Continental collision",
  "Transform boundaries",
  "What moves the plates",
  "Mantle convection",
  "Slab pull and ridge push",
];

/**
 * Opens the export route of the library's Plate tectonics in the running
 * app: the fake core lives in the page, so the route is reached without a
 * reload.
 */
async function openExport(page: Page) {
  await openApp(page, { colorScheme: "dark" });
  const row = page.getByRole("button", { name: "Plate tectonics" });
  const courseId = await row.getAttribute("data-course-open");
  if (courseId === null) throw new Error("the library row has no course id");
  await page.evaluate((id) => {
    window.location.hash = `#/export/${id}`;
  }, courseId);
  await expect(page.locator("html")).toHaveAttribute(
    "data-export-state",
    "ready",
    { timeout: 20_000 },
  );
}

test("in a dark app the export route renders in the light theme", async ({
  page,
}) => {
  await openExport(page);

  const title = page.getByRole("heading", {
    level: 1,
    name: "Plate tectonics",
  });
  await expect(title).toBeVisible();
  const {
    ground,
    ink,
    page: pageScheme,
  } = await title.evaluate((element) => {
    let ground = "";
    for (let node: Element | null = element; node; node = node.parentElement) {
      const background = getComputedStyle(node).backgroundColor;
      if (background !== "rgba(0, 0, 0, 0)") {
        ground = background;
        break;
      }
    }
    return {
      ground,
      ink: getComputedStyle(element).color,
      page: document.documentElement.dataset.theme,
    };
  });
  expect(pageScheme).toBe("dark");
  expect(distance(ground, LIGHT_CANVAS)).toBeLessThan(0.01);
  expect(wcagContrast(ink, ground)).toBeGreaterThan(10);
});

test("the contents list every index entry in index order", async ({ page }) => {
  await openExport(page);

  const entries = page
    .getByRole("navigation", { name: "Contents" })
    .getByRole("listitem");
  await expect(entries).toHaveCount(contents.length);
  for (const [at, title] of contents.entries()) {
    await expect(entries.nth(at)).toContainText(title);
  }
});

test("printing the export route gives a PDF of more than one page", async ({
  page,
}) => {
  await openExport(page);

  const pdf = await page.pdf({ format: "A4" });
  const pages = pdf.toString("latin1").match(/\/Type\s*\/Page(?![a-zA-Z])/g);
  expect(pages?.length ?? 0).toBeGreaterThan(1);
});
