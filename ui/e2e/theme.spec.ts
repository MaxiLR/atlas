import { expect, test, type Page } from "@playwright/test";
import { differenceEuclidean, wcagContrast } from "culori";

/** Distance in OKLab, the Cartesian form of OKLCH. */
const distance = differenceEuclidean("oklab");

/** The canvas is the theme's base colour, so it must arrive almost exact. */
const canvas = { dark: "#08090a", light: "#fdfdfd" } as const;
const raised = { dark: "#17181a", light: "#ffffff" } as const;
const STORAGE_KEY = "atlas-theme";

type Scheme = keyof typeof canvas;

async function styleOf(page: Page, selector: string) {
  return page.locator(selector).evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      background: style.backgroundColor,
      color: style.color,
      colorScheme: style.colorScheme,
    };
  });
}

/** How far the page's ground is from a theme's canvas. */
async function groundDistance(page: Page, scheme: Scheme) {
  const { background } = await styleOf(page, "html");
  return distance(background, canvas[scheme]);
}

async function expectPageTheme(page: Page, scheme: Scheme) {
  await expect.poll(() => groundDistance(page, scheme)).toBeLessThan(0.003);
  expect((await styleOf(page, "html")).colorScheme).toBe(scheme);
}

function storeMode(page: Page, mode: string) {
  return page.addInitScript(
    ([key, value]) => localStorage.setItem(key, value),
    [STORAGE_KEY, mode] as const,
  );
}

for (const scheme of ["light", "dark"] as const) {
  test(`with no stored choice the page follows a ${scheme} system`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/");
    await expectPageTheme(page, scheme);
  });
}

test("a stored choice wins over the system scheme", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await storeMode(page, "light");
  await page.goto("/");
  await expectPageTheme(page, "light");

  await page.emulateMedia({ colorScheme: "light" });
  await storeMode(page, "dark");
  await page.reload();
  await expectPageTheme(page, "dark");
});

test("in system mode the page follows a change of the system scheme", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await storeMode(page, "system");
  await page.goto("/");
  await expectPageTheme(page, "dark");

  await page.emulateMedia({ colorScheme: "light" });
  await expectPageTheme(page, "light");

  await page.emulateMedia({ colorScheme: "dark" });
  await expectPageTheme(page, "dark");
});

test("the theme is in place before the interface script runs", async ({
  page,
}) => {
  // Without the app bundle nothing but the document itself can paint the
  // theme, so this is the state of the first frame.
  await page.route(/\/src\/main\.tsx/, (route) => route.abort());
  await page.emulateMedia({ colorScheme: "light" });
  await storeMode(page, "dark");
  await page.goto("/");
  await expectPageTheme(page, "dark");
});

test("a subtree can be forced to the light theme", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await page.evaluate(() => {
    const scope = document.createElement("section");
    scope.id = "forced";
    scope.dataset.theme = "light";
    scope.textContent = "Exported page";
    document.body.append(scope);
  });

  const style = await styleOf(page, "#forced");
  expect(distance(style.background, canvas.light)).toBeLessThan(0.003);
  expect(style.colorScheme).toBe("light");
  expect(wcagContrast(style.color, style.background)).toBeGreaterThan(10);
  await expectPageTheme(page, "dark");
});

for (const scheme of ["light", "dark"] as const) {
  test(`a raised surface in the ${scheme} theme takes tokens derived from its own ground`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/");
    await page.evaluate(() => {
      const surface = document.createElement("div");
      surface.id = "raised";
      surface.dataset.surface = "raised";
      const label = document.createElement("span");
      label.className = "text-muted-foreground";
      label.textContent = "Explain differently";
      surface.append(label);
      document.body.append(surface);
    });

    const surface = await styleOf(page, "#raised");
    const label = await styleOf(page, "#raised span");
    expect(distance(surface.background, raised[scheme])).toBeLessThan(0.01);
    expect(
      wcagContrast(label.color, surface.background),
    ).toBeGreaterThanOrEqual(4.5);
  });
}

test("fonts are bundled and no request leaves the page for them", async ({
  page,
}) => {
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await page.goto("/");
  await expect(page.getByRole("textbox", { name: "Topic" })).toBeVisible();

  const faces = await page.evaluate(async () => {
    const loaded = await Promise.all([
      document.fonts.load('400 16px "Inter Variable"'),
      document.fonts.load('510 16px "Inter Variable"'),
      document.fonts.load('400 12px "IBM Plex Mono"'),
    ]);
    return loaded.map((faces) => faces.length);
  });
  expect(faces.every((count) => count > 0)).toBe(true);

  const body = await page.evaluate(() => {
    const style = getComputedStyle(document.body);
    return { family: style.fontFamily, features: style.fontFeatureSettings };
  });
  expect(body.family).toMatch(/^"Inter Variable"/);
  expect(body.features).toContain('"cv01"');
  expect(body.features).toContain('"ss03"');

  const origin = new URL(page.url()).origin;
  const outside = requests.filter(
    (url) => url.startsWith("http") && new URL(url).origin !== origin,
  );
  expect(outside).toEqual([]);
});

async function transitionOf(page: Page) {
  await page.evaluate(() => {
    const control = document.createElement("button");
    control.id = "moving";
    control.className = "transition-colors";
    control.textContent = "Next";
    document.body.append(control);
  });
  return page.locator("#moving").evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      duration: style.transitionDuration,
      easing: style.transitionTimingFunction,
    };
  });
}

test("transitions run 150 ms ease-out", async ({ page }) => {
  await page.goto("/");
  expect(await transitionOf(page)).toEqual({
    duration: "0.15s",
    easing: "ease-out",
  });
});

test("transitions stop under reduced motion", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  expect((await transitionOf(page)).duration).toBe("0s");
});
