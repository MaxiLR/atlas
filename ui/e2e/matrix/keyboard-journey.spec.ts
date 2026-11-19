import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { openApp } from "../support/app.ts";

/** What the focused element shows, as the journey checks it after a step. */
interface FocusReport {
  /** The focused element, as a person would name it in a failure. */
  focused: string;
  /** It is drawn, not covered and not clipped away. */
  visible: boolean;
  /** It lies wholly inside the window. */
  inWindow: boolean;
  /** It, or the frame drawn around it, shows the focus colour. */
  indicator: boolean;
  /** The page is no wider than the window. */
  horizontalScroll: boolean;
}

/**
 * Reads where focus is: an input drawn by something else (visually hidden,
 * or transparent over its option row) is seen through its label or parent;
 * the indicator is an outline in the focus colour, or a ring in it, on the
 * element or one of the three elements around it.
 */
function focusReport(page: Page): Promise<FocusReport> {
  return page.evaluate(() => {
    const root = document.documentElement;
    const horizontalScroll =
      root.scrollWidth > root.clientWidth ||
      document.body.scrollWidth > root.clientWidth;
    const focused = document.activeElement;
    if (!(focused instanceof HTMLElement) || focused === document.body) {
      return {
        focused: "nothing",
        visible: false,
        inWindow: false,
        indicator: false,
        horizontalScroll,
      };
    }
    const name =
      focused.getAttribute("aria-label") ??
      focused.textContent?.trim().slice(0, 60) ??
      "";
    const described = `${focused.tagName.toLowerCase()} “${name}”`;

    const own = focused.getBoundingClientRect();
    const drawnElsewhere =
      own.width <= 1 ||
      own.height <= 1 ||
      getComputedStyle(focused).opacity === "0";
    const shown = drawnElsewhere
      ? (focused.closest("label") ?? focused.parentElement ?? focused)
      : focused;
    const rect = shown.getBoundingClientRect();
    const hit = document.elementFromPoint(
      rect.left + rect.width / 2,
      rect.top + Math.min(rect.height / 2, 12),
    );
    const visible =
      rect.width > 1 &&
      rect.height > 1 &&
      shown.checkVisibility({
        opacityProperty: true,
        visibilityProperty: true,
      }) &&
      hit !== null &&
      (shown.contains(hit) || hit.contains(shown));
    const inWindow =
      rect.left >= -0.5 &&
      rect.top >= -0.5 &&
      rect.right <= root.clientWidth + 0.5 &&
      rect.bottom <= root.clientHeight + 0.5;

    const probe = document.createElement("span");
    probe.style.color = "var(--color-focus)";
    (shown.parentElement ?? shown).append(probe);
    const focusColour = getComputedStyle(probe).color;
    probe.remove();
    // A text field shows focus as its frame's border in the focus colour; a
    // row's control may draw it on a pseudo-element spanning the row.
    const field = focused.matches("input:not([type=radio]), textarea");
    let indicator = false;
    let node: Element | null = focused;
    for (let depth = 0; node !== null && depth < 4; depth += 1) {
      for (const pseudo of [null, "::before", "::after"]) {
        const style = getComputedStyle(node, pseudo);
        if (
          (style.outlineStyle !== "none" &&
            parseFloat(style.outlineWidth) >= 1 &&
            style.outlineColor === focusColour) ||
          style.boxShadow.includes(focusColour) ||
          (field && style.borderTopColor === focusColour)
        ) {
          indicator = true;
        }
      }
      if (indicator) break;
      node = node.parentElement;
    }
    return {
      focused: described,
      visible,
      inWindow,
      indicator,
      horizontalScroll,
    };
  });
}

/** Focus sits on a visible element, in the window, drawn with its indicator. */
async function expectFocusShown(page: Page) {
  await expect
    .poll(() => focusReport(page))
    .toEqual({
      focused: expect.any(String),
      visible: true,
      inWindow: true,
      indicator: true,
      horizontalScroll: false,
    });
}

/** Presses `key`, then checks where focus went. */
async function press(page: Page, key: string) {
  await page.keyboard.press(key);
  await expectFocusShown(page);
}

/** Moves focus with `key` (Tab by default) until `target` has it. */
async function moveTo(page: Page, target: Locator, key = "Tab") {
  for (let step = 0; step < 120; step += 1) {
    if (await target.evaluate((element) => element === document.activeElement))
      return;
    await press(page, key);
  }
  throw new Error(`${key} never reached ${target.toString()}`);
}

/** The project's width and colour scheme. */
function projectOf(testInfo: {
  project: {
    use: { viewport?: { width: number } | null; colorScheme?: unknown };
  };
}) {
  const { viewport, colorScheme } = testInfo.project.use;
  return {
    width: viewport?.width ?? 1440,
    colorScheme: (colorScheme ?? "dark") as "light" | "dark",
  };
}

/** No contrast or name violations, in either theme. */
async function expectReadable(page: Page) {
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa"])
    .analyze();
  const named = results.violations.filter(
    (violation) =>
      violation.id === "color-contrast" ||
      /name|label|alt|title/.test(violation.id),
  );
  expect(
    named.map((violation) => ({
      id: violation.id,
      targets: violation.nodes.map((node) => node.target.join(" ")),
    })),
  ).toEqual([]);
}

/**
 * The core loop by keyboard alone: sign in, name a topic, placement, the
 * index, a streamed section, Next, Go deeper and a deep dive, Define, Ask
 * and its reply, the library and resume.
 */
async function journey(page: Page, width: number) {
  const narrow = width < 1024;
  const index = page.getByRole("navigation", { name: "Index" });
  const indexToggle = page.getByRole("button", { name: "Show the index" });

  // Signing in.
  const signIn = page.getByRole("button", { name: "Continue with ChatGPT" });
  await expect(signIn).toBeVisible();
  await moveTo(page, signIn);
  await press(page, "Enter");

  // The library opens with focus in the topic field.
  const topic = page.getByRole("textbox", { name: "Topic" });
  await expect(topic).toBeFocused();
  await expectFocusShown(page);
  await expectReadable(page);
  await page.keyboard.type("Plate tectonics");
  await press(page, "Enter");

  // Placement: the self-rating, the goal, then four questions.
  const ratings = page.getByRole("group", { name: "How well do you know it?" });
  await expect(ratings).toBeFocused();
  await expectFocusShown(page);
  await moveTo(page, ratings.getByRole("radio", { name: "New to this" }));
  await press(page, "ArrowDown");
  await expect(
    ratings.getByRole("radio", { name: "Know the basics" }),
  ).toBeChecked();
  const goals = page.getByRole("group", { name: "What is your goal?" });
  await moveTo(page, goals.getByRole("radio", { name: "Overview" }));
  await press(page, "ArrowDown");
  await expect(
    goals.getByRole("radio", { name: "Working knowledge" }),
  ).toBeChecked();
  await press(page, "Enter");

  for (const [number, key] of ["1", "2", "0", "1"].entries()) {
    await expect(
      page.getByText(`Question ${number + 1} of 4 to 6`),
    ).toBeVisible();
    await expectFocusShown(page);
    await press(page, key);
    await press(page, "Enter");
  }

  // The index, and the first section opened with focus on its title.
  await expect(index).toBeVisible();
  const first = page.getByRole("article", { name: "Layers by composition" });
  await expect(first.getByRole("heading", { level: 1 })).toBeFocused();
  await expectFocusShown(page);
  await expect(first).toContainText("roughly what the skin is to an apple.");

  // Another section from the index; it streams.
  if (narrow) {
    await moveTo(page, indexToggle, "Shift+Tab");
    await press(page, "Enter");
  } else {
    await moveTo(
      page,
      index.getByRole("button", { name: "Layers by composition" }),
      "Shift+Tab",
    );
  }
  await expect(
    index.getByRole("button", { name: "Layers by composition" }),
  ).toBeFocused();
  await press(page, "ArrowDown");
  await press(page, "Enter");
  const second = page.getByRole("article", {
    name: "Lithosphere and asthenosphere",
  });
  await expect(second.getByRole("heading", { level: 1 })).toBeFocused();
  await expectFocusShown(page);
  await expect(second).toContainText("and the plates ride on it.");

  // Next, from the foot.
  const next = page.getByRole("button", { name: "Next", exact: true });
  await moveTo(page, next);
  await press(page, "Enter");
  const third = page.getByRole("article", { name: "Divergent boundaries" });
  await expect(third).toBeVisible();
  await expectFocusShown(page);
  await expect(third).toContainText("widens the Atlantic");
  // The column starts at the top of the next section.
  await expect(third.getByRole("heading", { level: 1 })).toBeInViewport();

  // Go deeper proposes deep dives under the section; one opens from the index.
  const goDeeper = page.getByRole("button", { name: "Go deeper", exact: true });
  await moveTo(page, goDeeper, "Shift+Tab");
  await press(page, "Enter");
  const deepDive = index.getByRole("button", {
    name: "Divergent boundaries: measured",
  });
  await expect(deepDive).toHaveCount(1);
  await expectFocusShown(page);
  if (narrow) {
    await moveTo(page, indexToggle, "Shift+Tab");
    await press(page, "Enter");
  } else {
    await moveTo(
      page,
      index.getByRole("button", { name: "Divergent boundaries", exact: true }),
      "Shift+Tab",
    );
  }
  await moveTo(page, deepDive, "ArrowDown");
  await press(page, "Enter");
  const dive = page.getByRole("article", {
    name: "Divergent boundaries: measured",
  });
  await expect(dive.getByRole("heading", { level: 1 })).toBeFocused();
  await expectFocusShown(page);
  await expect(dive).toContainText("This deep dive looks closer");
  await expect(dive).toHaveAttribute("aria-busy", "false");

  // A selection made with the keyboard: a word, then the next one; Define.
  const selectedText = () =>
    page.evaluate(() => document.getSelection()?.toString().trim() ?? "");
  await page.keyboard.press("Shift+ArrowRight");
  await page.keyboard.press("Alt+Shift+ArrowRight");
  const selection = page.getByRole("toolbar", { name: "Selection" });
  await expect(selection).toBeVisible();
  await expect.poll(selectedText).toMatch(/^\S+\s+\S+$/);
  const selected = await selectedText();
  await press(page, "d");
  const definition = page.getByRole("dialog", { name: /^Definition of / });
  await expect(definition).toBeVisible();
  await expect(definition).toHaveAccessibleName(`Definition of ${selected}`);
  await expect(definition).toBeFocused();
  await expectFocusShown(page);
  // Read through, then closed: focus goes back to the term in the text.
  await expect(definition).toHaveAttribute("aria-busy", "false");
  await press(page, "Escape");
  await expect(definition).toBeHidden();
  const term = dive.getByRole("button", { name: selected, exact: true });
  await expect(term).toBeFocused();
  await expectFocusShown(page);

  // Ask about a passage, and get a reply beside it.
  await page.keyboard.press("Shift+ArrowRight");
  await page.keyboard.press("Alt+Shift+ArrowRight");
  await expect(selection).toBeVisible();
  await press(page, "a");
  const panel = page.getByRole("complementary", { name: "Side panel" });
  const field = panel.getByRole("textbox", { name: "Ask a question" });
  await expect(field).toBeFocused();
  await expectFocusShown(page);
  await page.keyboard.type("Why do plates move at all?");
  await press(page, "Enter");
  await expect(panel).toContainText(
    "the cold, downward limb of the mantle's circulation.",
  );
  await expectFocusShown(page);
  await expectReadable(page);
  await press(page, "Escape");
  await expect(panel).toBeHidden();
  await expectFocusShown(page);

  // Back to the library, then resume where the course was left.
  const library = page.getByRole("button", { name: "Library" });
  if (narrow) {
    await moveTo(page, indexToggle, "Shift+Tab");
    await press(page, "Enter");
  }
  await moveTo(page, library, "Shift+Tab");
  await press(page, "Enter");
  await expect(topic).toBeFocused();
  await expectFocusShown(page);
  await moveTo(page, page.getByRole("button", { name: "Plate tectonics" }));
  await press(page, "Enter");
  const resumed = page.getByRole("article", {
    name: "Divergent boundaries: measured",
  });
  await expect(resumed.getByRole("heading", { level: 1 })).toBeFocused();
  await expectFocusShown(page);
}

test("the core loop by keyboard alone, focus always shown and in the window", async ({
  page,
}, testInfo) => {
  const { width, colorScheme } = projectOf(testInfo);
  await openApp(page, { scenario: "first-launch", width, colorScheme });
  await journey(page, width);
});

test("the core loop by keyboard alone with reduced motion", async ({
  page,
}, testInfo) => {
  const { width, colorScheme } = projectOf(testInfo);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openApp(page, { scenario: "first-launch", width, colorScheme });
  await journey(page, width);
});

/** Opens Plate tectonics from the library by keyboard; focus on its title. */
async function resumeSample(page: Page) {
  const course = page.getByRole("button", { name: "Plate tectonics" });
  await expect(course).toBeVisible();
  await moveTo(page, course);
  await press(page, "Enter");
  const article = page.getByRole("article", { name: "Convergent boundaries" });
  const title = article.getByRole("heading", { level: 1 });
  await expect(title).toBeFocused();
  return title;
}

test("the skip control at the start of the rail shows when focused and leads to the section's title", async ({
  page,
}, testInfo) => {
  const { width, colorScheme } = projectOf(testInfo);
  await openApp(page, { width, colorScheme });
  const title = await resumeSample(page);

  const skip = page.getByRole("link", { name: "Skip to the reading column" });
  await moveTo(page, skip, "Shift+Tab");
  await expect(skip).toBeVisible();
  await press(page, "Enter");

  await expect(title).toBeFocused();
});

test("below 1024 px the titles overlay and the side panel hold focus while open, over inert content", async ({
  page,
}, testInfo) => {
  const { width, colorScheme } = projectOf(testInfo);
  test.skip(width >= 1024, "the overlays open over the column below 1024 px");
  await openApp(page, { width, colorScheme });
  await resumeSample(page);
  const rail = page.getByRole("complementary", { name: "Course" });
  const main = page.getByRole("main");
  const focusIn = (region: typeof rail) =>
    region.evaluate((element) => element.contains(document.activeElement));

  // The titles overlay.
  const toggle = page.getByRole("button", { name: "Show the index" });
  await moveTo(page, toggle, "Shift+Tab");
  await press(page, "Enter");
  await expect(main).toHaveAttribute("inert");
  for (const key of ["Tab", "Shift+Tab"]) {
    for (let step = 0; step < 25; step += 1) {
      await press(page, key);
      expect(await focusIn(rail)).toBe(true);
    }
  }
  await press(page, "Escape");
  await expect(toggle).toBeFocused();
  await expect(main).not.toHaveAttribute("inert");

  // The side panel.
  const chats = page.getByRole("button", { name: /^Side chats/ });
  await moveTo(page, chats);
  await press(page, "Enter");
  const panel = page.getByRole("complementary", { name: "Side panel" });
  await expect(panel).toBeVisible();
  await expect(main).toHaveAttribute("inert");
  await expect(rail).toHaveAttribute("inert");
  for (const key of ["Tab", "Shift+Tab"]) {
    for (let step = 0; step < 25; step += 1) {
      await press(page, key);
      expect(await focusIn(panel)).toBe(true);
    }
  }
  await press(page, "Escape");
  await expect(panel).toBeHidden();
  await expect(chats).toBeFocused();
  await expect(main).not.toHaveAttribute("inert");
});
