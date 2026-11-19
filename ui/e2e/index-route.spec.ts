import { expect, test, type Locator, type Page } from "@playwright/test";

import { openApp } from "./support/app.ts";

async function box(locator: Locator) {
  const found = await locator.boundingBox();
  if (found === null) throw new Error("not rendered");
  return found;
}

/** Opens the course on `topic` from the library; returns its index. */
async function openCourse(page: Page, topic: string) {
  await page.getByRole("button", { name: topic }).click();
  const index = page.getByRole("navigation", { name: "Index" });
  await expect(index).toBeVisible();
  return index;
}

/** The row named `title`, exactly. */
function row(index: Locator, title: string) {
  return index.getByRole("button", { name: title, exact: true });
}

/** The node drawn for a row. */
function node(index: Locator, title: string) {
  return row(index, title).locator("[data-mark] circle").first();
}

/**
 * Starts recording, on every frame, which parts of the index animate: the
 * line's `data-line` name, or the tag of anything else. The names gather on
 * the root element's `data-index-animations`.
 */
async function recordIndexAnimations(page: Page) {
  await page.evaluate(() => {
    const seen = new Set<string>();
    const look = () => {
      for (const animation of document.getAnimations()) {
        const { effect } = animation;
        const target = effect instanceof KeyframeEffect ? effect.target : null;
        if (target?.closest('nav[aria-label="Index"]')) {
          seen.add(target.getAttribute("data-line") ?? target.tagName);
        }
      }
      document.documentElement.dataset.indexAnimations = [...seen].join(" ");
      requestAnimationFrame(look);
    };
    look();
  });
  return () =>
    page.evaluate(() =>
      (document.documentElement.dataset.indexAnimations ?? "")
        .split(" ")
        .filter((name) => name !== ""),
    );
}

/** A computed style property of an element. */
function style(locator: Locator, property: string) {
  return locator.evaluate(
    (element, name) => getComputedStyle(element).getPropertyValue(name),
    property,
  );
}

test("at 1440 the open section's row is current and the arrow keys move along the rows", async ({
  page,
}) => {
  await openApp(page, { width: 1440 });
  const index = await openCourse(page, "Plate tectonics");
  await row(index, "Convergent boundaries").click();
  await expect(row(index, "Convergent boundaries")).toHaveAttribute(
    "aria-current",
    "page",
  );

  await page.getByRole("button", { name: "Library" }).focus();
  await page.keyboard.press("Tab");
  await expect(row(index, "Convergent boundaries")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(row(index, "Subduction, step by step")).toBeFocused();
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowUp");
  await expect(row(index, "Divergent boundaries")).toBeFocused();
  await page.keyboard.press("Enter");

  await expect(
    page.getByRole("article", { name: "Divergent boundaries" }),
  ).toBeVisible();
  await expect(row(index, "Divergent boundaries")).toHaveAttribute(
    "aria-current",
    "page",
  );
  await expect(index.locator('[aria-current="page"]')).toHaveCount(1);
});

test("selecting a chapter by keyboard opens its first section", async ({
  page,
}) => {
  await openApp(page, { width: 1440 });
  const index = await openCourse(page, "Plate tectonics");

  await page.getByRole("button", { name: "Library" }).focus();
  await page.keyboard.press("Tab");
  await expect(row(index, "The solid Earth")).toBeFocused();
  for (let step = 0; step < 3; step += 1)
    await page.keyboard.press("ArrowDown");
  await expect(row(index, "Plates and their boundaries")).toBeFocused();
  await page.keyboard.press("Enter");

  await expect(
    page.getByRole("article", { name: "Divergent boundaries" }),
  ).toBeVisible();
  await expect(row(index, "Divergent boundaries")).toHaveAttribute(
    "aria-current",
    "page",
  );
});

test("a fresh course shows its next chapter's sections and the one after as a station with its count", async ({
  page,
}) => {
  await openApp(page, { width: 1440 });
  await page.getByRole("textbox", { name: "Topic" }).fill("Volcanoes");
  await page.getByRole("button", { name: "Start course" }).click();
  await page.getByRole("radio", { name: "New to this" }).check();
  await page.getByRole("radio", { name: "Overview" }).check();
  await page.getByRole("button", { name: "Continue" }).click();
  for (let number = 1; number <= 4; number += 1) {
    await expect(page.getByText(`Question ${number} of 4 to 6`)).toBeVisible();
    await page.getByRole("radio", { name: "Not sure" }).check();
    await page.getByRole("button", { name: "Continue" }).click();
  }

  const index = page.getByRole("navigation", { name: "Index" });
  await expect(index.getByRole("button")).toHaveCount(8);
  const station = row(index, "What moves the plates");
  await expect(station).toHaveAccessibleDescription("Chapter, 2 sections");
  await expect(station).toContainText("2");
  await expect(row(index, "Mantle convection")).toHaveCount(0);

  await station.click();
  await expect(
    page.getByRole("article", { name: "Mantle convection" }),
  ).toBeVisible();
  await expect(row(index, "Mantle convection")).toHaveAttribute(
    "aria-current",
    "page",
  );
  await expect(row(index, "Slab pull and ridge push")).toBeVisible();
});

test("after Go deeper the new nodes branch from the section, drawn along the connector", async ({
  page,
}) => {
  await openApp(page, { width: 1440 });
  const index = await openCourse(page, "Plate tectonics");
  await row(index, "Divergent boundaries").click();
  await expect(
    page.getByRole("article", { name: "Divergent boundaries" }),
  ).toBeVisible();
  const animated = await recordIndexAnimations(page);

  await page.getByRole("button", { name: "Go deeper" }).click();

  const parent = await box(node(index, "Divergent boundaries"));
  const next = await box(node(index, "Convergent boundaries"));
  for (const title of [
    "Divergent boundaries: in the field",
    "Divergent boundaries: measured",
    "Divergent boundaries: open questions",
  ]) {
    await expect(row(index, title)).toBeVisible();
    const child = await box(node(index, title));
    expect(child.x).toBeGreaterThan(parent.x);
    expect(child.y).toBeGreaterThan(parent.y);
    expect(child.y).toBeLessThan(next.y);
  }
  await expect(row(index, "Divergent boundaries")).toHaveAccessibleDescription(
    "Read, 3 deep dives",
  );
  await expect.poll(animated).toEqual(expect.arrayContaining(["branch"]));
});

test("after Go deeper the new nodes sit between the section and the next one, drawn along the connector", async ({
  page,
}) => {
  await openApp(page, { width: 1440 });
  const index = await openCourse(page, "Plate tectonics");
  await row(index, "Divergent boundaries").click();
  await expect(
    page.getByRole("article", { name: "Divergent boundaries" }),
  ).toBeVisible();
  const animated = await recordIndexAnimations(page);

  await page.getByRole("button", { name: "Go deeper" }).click();
  await expect(
    row(index, "Divergent boundaries: open questions"),
  ).toBeVisible();

  const parent = await box(node(index, "Divergent boundaries"));
  const next = await box(node(index, "Convergent boundaries"));
  for (const title of [
    "Divergent boundaries: in the field",
    "Divergent boundaries: measured",
    "Divergent boundaries: open questions",
  ]) {
    await expect(row(index, title)).toBeVisible();
    const child = await box(node(index, title));
    expect(child.x).toBeGreaterThan(parent.x);
    expect(child.y).toBeGreaterThan(parent.y);
    expect(child.y).toBeLessThan(next.y);
  }
  await expect(row(index, "Divergent boundaries")).toHaveAccessibleDescription(
    "Read, 3 deep dives",
  );
  await expect.poll(animated).toEqual(expect.arrayContaining(["branch"]));
});

test("Next advances the bright part of the line", async ({ page }) => {
  await openApp(page, { width: 1440 });
  const index = await openCourse(page, "Plate tectonics");
  await row(index, "Convergent boundaries").click();
  const travelled = index.locator('[data-line="travelled"]');
  const before = await box(travelled);
  const animated = await recordIndexAnimations(page);

  await page.getByRole("button", { name: "Next" }).click();
  const article = page.getByRole("article", { name: "Transform boundaries" });
  await expect(article).toContainText("we feel as earthquakes.");

  await expect(row(index, "Transform boundaries")).toHaveAccessibleDescription(
    "Read",
  );
  await expect
    .poll(async () => (await box(travelled)).height)
    .toBeGreaterThan(before.height);
  expect(await animated()).toEqual(expect.arrayContaining(["travelled"]));
});

test("at 720 the rail shows nodes without titles and the overlay lists them, drawn the same way", async ({
  page,
}) => {
  await openApp(page, { width: 720 });
  const index = await openCourse(page, "Plate tectonics");
  await row(index, "Convergent boundaries").click();

  for (const title of ["The solid Earth", "Transform boundaries"]) {
    await expect(row(index, title)).toBeVisible();
    expect((await box(row(index, title))).width).toBeLessThanOrEqual(72);
    await expect(node(index, title)).toBeVisible();
    await expect(index.getByText(title, { exact: true })).toBeHidden();
  }
  await expect(row(index, "Convergent boundaries")).toHaveAttribute(
    "aria-current",
    "page",
  );
  const line = index.locator('[data-line="rest"]');
  await expect(line).toBeVisible();

  await page.getByRole("button", { name: "Show the index" }).click();
  for (const title of ["The solid Earth", "Transform boundaries"]) {
    await expect(index.getByText(title, { exact: true })).toBeVisible();
    await expect(node(index, title)).toBeVisible();
  }
  await expect(line).toBeVisible();
  await expect(index.locator('[data-line="branch"]').first()).toBeVisible();
});

test("with reduced motion nothing in the index animates and the branch is drawn at once", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openApp(page, { width: 1440 });
  const index = await openCourse(page, "Plate tectonics");
  await row(index, "Divergent boundaries").click();
  await expect(
    page.getByRole("article", { name: "Divergent boundaries" }),
  ).toBeVisible();
  const animated = await recordIndexAnimations(page);

  await page.getByRole("button", { name: "Go deeper" }).click();
  await expect(
    row(index, "Divergent boundaries: open questions"),
  ).toBeVisible();
  await page.getByRole("button", { name: "Next" }).click();
  await expect(
    page.getByRole("article", { name: "Convergent boundaries" }),
  ).toBeVisible();
  // Several frames after both changes.
  await page.waitForTimeout(400);

  expect(await animated()).toEqual([]);
  const branch = index.locator('[data-line="branch"]').first();
  await expect(branch).toBeVisible();
  expect(
    Number.parseFloat((await style(branch, "stroke-dashoffset")) || "0"),
  ).toBe(0);
});

for (const scheme of ["light", "dark"] as const) {
  test(`states differ in shape, not only colour, in the ${scheme} theme`, async ({
    page,
  }) => {
    await openApp(page, { width: 1440, colorScheme: scheme });
    const index = await openCourse(page, "Plate tectonics");
    await row(index, "Convergent boundaries").click();
    const ground = await style(
      page.getByRole("complementary", { name: "Course" }),
      "background-color",
    );

    // Read: a filled disc with no ring.
    const read = node(index, "Layers by composition");
    expect(await style(read, "stroke")).toBe("none");
    expect(await style(read, "fill")).not.toBe(ground);
    // Unread: a ring around the rail's own ground, hollow.
    const unread = node(index, "Transform boundaries");
    expect(await style(unread, "stroke")).not.toBe("none");
    expect(
      Number.parseFloat(await style(unread, "stroke-width")),
    ).toBeGreaterThan(0);
    expect(await style(unread, "fill")).toBe(ground);
    // Same size: the difference is the fill, not the hue.
    expect((await box(read)).width).toBeCloseTo((await box(unread)).width, 0);

    // A branch: a connector leaves the node and runs down to its deep dives.
    const convergent = await box(node(index, "Convergent boundaries"));
    const branch = await box(index.locator('[data-line="branch"]').first());
    expect(branch.y).toBeGreaterThanOrEqual(convergent.y);
    expect(branch.x + branch.width).toBeGreaterThan(
      convergent.x + convergent.width,
    );
    // The current node: a ring with a dot inside it, larger than any other.
    const current = row(index, "Convergent boundaries").locator(
      "[data-mark] circle",
    );
    await expect(current).toHaveCount(2);
    expect((await box(current.first())).width).toBeGreaterThan(
      (await box(read)).width,
    );
    // A chapter station: a larger ring than a section's node.
    const station = node(index, "Plates and their boundaries");
    expect(await style(station, "fill")).toBe(ground);
    expect((await box(station)).width).toBeGreaterThan(
      (await box(unread)).width,
    );
  });
}

test("a long course scrolls inside the rail, cuts long titles short and keeps the open node in view", async ({
  page,
}) => {
  await openApp(page, { width: 1440, scenario: "index-route" });
  const index = await openCourse(page, "The history of mapmaking");

  const long = row(
    index,
    "How a clay tablet from Babylon placed the known world inside a ring of ocean, and what that shape tells us",
  );
  expect((await box(long)).height).toBe(30);
  const label = long.getByText(/^How a clay tablet/);
  expect(
    await label.evaluate(
      (element) => element.scrollWidth > element.clientWidth,
    ),
  ).toBe(true);

  expect(
    await index.evaluate(
      (element) => element.scrollHeight > element.clientHeight,
    ),
  ).toBe(true);
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollHeight <=
        document.documentElement.clientHeight,
    ),
  ).toBe(true);

  await row(index, "Ptolemy and the grid: legacy").click();
  await index.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(row(index, "Ptolemy and the grid: legacy")).not.toBeInViewport();

  await page.getByRole("button", { name: "Next" }).click();
  const opened = row(
    index,
    "Measuring the Earth from Eratosthenes' shadows to the triangulation surveys of the eighteenth century: sources",
  );
  await expect(opened).toHaveAttribute("aria-current", "page");
  await expect(opened).toBeInViewport();
});

test("a long course keeps the open node in view after the rail is scrolled away from it", async ({
  page,
}) => {
  await openApp(page, { width: 1440, scenario: "index-route" });
  const index = await openCourse(page, "The history of mapmaking");

  const long = row(
    index,
    "How a clay tablet from Babylon placed the known world inside a ring of ocean, and what that shape tells us",
  );
  expect((await box(long)).height).toBe(30);
  const label = long.getByText(/^How a clay tablet/);
  expect(
    await label.evaluate(
      (element) => element.scrollWidth > element.clientWidth,
    ),
  ).toBe(true);

  expect(
    await index.evaluate(
      (element) => element.scrollHeight > element.clientHeight,
    ),
  ).toBe(true);
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollHeight <=
        document.documentElement.clientHeight,
    ),
  ).toBe(true);

  await row(index, "Maps on every phone").click();
  const first = row(index, "Maps on every phone: sources");
  await expect(first).toHaveAttribute("aria-current", "page");
  await expect(first).toBeInViewport();
  await expect(
    page.getByRole("article", { name: "Maps on every phone: sources" }),
  ).toContainText("is part of the history of mapmaking.");

  await index.evaluate((element) => {
    element.scrollTop = 0;
  });
  await expect(first).not.toBeInViewport();

  await page.getByRole("button", { name: "Next" }).click();
  const opened = row(index, "Maps on every phone: instruments");
  await expect(opened).toHaveAttribute("aria-current", "page");
  await expect(opened).toBeInViewport();
});
