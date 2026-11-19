import { expect, test } from "@playwright/test";

import { openApp } from "./support/app.ts";

const TITLE = "Kepler's third law";
const FIRST = "A planet further from the Sun";

test("a streaming section grows below its first paragraph without moving it, and renders maths and code", async ({
  page,
  baseURL,
}) => {
  const outside: string[] = [];
  page.on("request", (request) => {
    if (!request.url().startsWith(baseURL!)) outside.push(request.url());
  });
  await openApp(page, { scenario: "section-blocks" });

  await page.getByRole("button", { name: "Kepler's laws" }).click();
  const index = page.getByRole("navigation", { name: "Index" });
  await index.getByRole("button", { name: TITLE }).click();
  const article = page.getByRole("article", { name: TITLE });
  await expect(article).toHaveAttribute("aria-busy", "true");
  await expect(article.getByText(FIRST)).toBeVisible();

  // Every frame until the section is complete: where the first paragraph
  // sits, and how many figures have arrived below it.
  const samples = await page.evaluate((first) => {
    const { promise, resolve } =
      Promise.withResolvers<{ top: number; figures: number }[]>();
    const taken: { top: number; figures: number }[] = [];
    const sample = () => {
      const article = document.querySelector("article");
      const paragraph = [...(article?.querySelectorAll("p") ?? [])].find(
        (element) => element.textContent?.startsWith(first),
      );
      if (
        article === null ||
        paragraph === undefined ||
        article.getAttribute("aria-busy") !== "true"
      ) {
        resolve(taken);
        return;
      }
      taken.push({
        top: paragraph.getBoundingClientRect().top,
        figures: article.querySelectorAll("figure").length,
      });
      requestAnimationFrame(sample);
    };
    sample();
    return promise;
  }, FIRST);

  expect(samples.length).toBeGreaterThan(10);
  expect(samples.at(-1)!.figures).toBeGreaterThan(samples[0]!.figures);
  expect(new Set(samples.map((sample) => sample.top)).size).toBe(1);

  await expect(article).toHaveAttribute("aria-busy", "false");
  await expect(article.locator("math")).toHaveCount(4);
  await expect(article.locator('math[display="block"]')).toHaveCount(1);
  await expect(article.getByText(/def period\(a, mass\):/)).toBeVisible();
  await expect(
    article.getByRole("figure", {
      name: "Fig. 1 An elliptical orbit with the Sun at one focus.",
    }),
  ).toBeVisible();
  expect(outside).toEqual([]);
});
