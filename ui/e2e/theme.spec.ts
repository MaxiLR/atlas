import { expect, test, type Page } from "@playwright/test";
import { converter, differenceEuclidean, wcagContrast } from "culori";

/** Distance in OKLab, the Cartesian form of OKLCH. */
const distance = differenceEuclidean("oklab");
const toOklch = converter("oklch");

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

const toRgb = converter("rgb");

/** A colour as it shows over an opaque ground. */
function over(top: string, ground: string) {
  const front = toRgb(top);
  const back = toRgb(ground);
  if (!front || !back) throw new Error(`Not a colour: ${top} or ${ground}`);
  const alpha = front.alpha ?? 1;
  const mix = (a: number, b: number) => a * alpha + b * (1 - alpha);
  return {
    mode: "rgb" as const,
    r: mix(front.r, back.r),
    g: mix(front.g, back.g),
    b: mix(front.b, back.b),
  };
}

/**
 * Puts markup into the page under a fresh scope: the page itself, or a
 * raised surface on it. Returns the scope's selector.
 */
async function mount(page: Page, raised: boolean, html: string) {
  return page.evaluate(
    ([isRaised, markup]) => {
      const scope = document.createElement("div");
      scope.id = `scope-${document.querySelectorAll("[id^=scope-]").length}`;
      if (isRaised) scope.dataset.surface = "raised";
      scope.innerHTML = markup;
      document.body.append(scope);
      return `#${scope.id}`;
    },
    [raised, html] as const,
  );
}

/** The three grounds text and marks sit on, as tokens of a scope. */
async function groundsOf(page: Page, scope: string) {
  return page.locator(scope).evaluate((element) => {
    const style = getComputedStyle(element);
    return ["canvas", "panel", "raised"].map((name) =>
      style.getPropertyValue(`--atlas-${name}`).trim(),
    );
  });
}

async function tokenOf(page: Page, scope: string, name: string) {
  return page
    .locator(scope)
    .evaluate(
      (element, token) => getComputedStyle(element).getPropertyValue(token),
      name,
    );
}

const scopes = [
  ["the page", false],
  ["a raised surface", true],
] as const;

for (const scheme of ["light", "dark"] as const) {
  for (const [where, raisedScope] of scopes) {
    test(`a highlighted menu row in ${where} is a neutral fill in the ${scheme} theme`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await page.goto("/");
      const scope = await mount(
        page,
        raisedScope,
        `<div id="row" tabindex="0" class="focus:bg-accent focus:text-accent-foreground">Rename course</div>`,
      );
      await page.locator(`${scope} #row`).focus();

      const ground = (await styleOf(page, scope)).background;
      const row = await styleOf(page, `${scope} #row`);
      const fill = over(row.background, ground);
      expect(distance(fill, ground)).toBeGreaterThan(0.01);
      expect(toOklch(fill)?.c ?? 1).toBeLessThan(0.02);
      expect(wcagContrast(row.color, fill)).toBeGreaterThanOrEqual(4.5);
    });

    test(`control boundaries and focus outlines in ${where} reach 3:1 in the ${scheme} theme`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await page.goto("/");
      const scope = await mount(
        page,
        raisedScope,
        `<span id="ring" class="border border-input"></span><button id="plain">Next</button>`,
      );
      await page.locator(`${scope} #plain`).focus();

      const marks = await page.locator(scope).evaluate((element) => ({
        boundary: getComputedStyle(element.querySelector("#ring")!)
          .borderTopColor,
        outline: getComputedStyle(element.querySelector("#plain")!)
          .outlineColor,
      }));
      for (const ground of await groundsOf(page, scope)) {
        for (const [name, color] of Object.entries(marks)) {
          expect(
            wcagContrast(over(color, ground), ground),
            `${name} on ${ground}`,
          ).toBeGreaterThanOrEqual(3);
        }
      }
    });

    test(`marked text in the reading column in ${where} takes the accent wash in the ${scheme} theme`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await page.goto("/");
      const scope = await mount(
        page,
        raisedScope,
        `<div class="typeset"><p>Slab pull is the <mark>larger force</mark>.</p></div>`,
      );

      const ground = (await styleOf(page, scope)).background;
      const mark = await styleOf(page, `${scope} mark`);
      const wash = await tokenOf(page, scope, "--atlas-accent-wash");
      const fill = over(mark.background, ground);
      expect(distance(fill, over(wash, ground))).toBeLessThan(0.01);
      expect(wcagContrast(mark.color, fill)).toBeGreaterThanOrEqual(4.5);
    });

    test(`a tinted chat bubble in ${where} takes the accent wash in the ${scheme} theme`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await page.goto("/");
      // The component itself, imported in the page where Vite serves it (the
      // test process cannot import it statically); it renders no hooks, so
      // calling it gives the element the app would mount.
      const className = await page.evaluate(async (path) => {
        const loaded = (await import(path)) as {
          Bubble: (props: { variant: string }) => {
            props: { className: string };
          };
        };
        return loaded.Bubble({ variant: "tinted" }).props.className;
      }, "/src/components/ui/bubble.tsx");
      const scope = await mount(
        page,
        raisedScope,
        `<div class="${className}"><div id="content" data-slot="bubble-content">Why does slab pull win?</div></div>`,
      );

      const ground = (await styleOf(page, scope)).background;
      const content = await styleOf(page, `${scope} #content`);
      const wash = await tokenOf(page, scope, "--atlas-accent-wash");
      const fill = over(content.background, ground);
      expect(distance(fill, over(wash, ground))).toBeLessThan(0.01);
      expect(wcagContrast(content.color, fill)).toBeGreaterThanOrEqual(4.5);
    });
  }
}

test("the reading column uses only the weights 400 and 510", async ({
  page,
}) => {
  await page.goto("/");
  const scope = await mount(
    page,
    false,
    `<div class="typeset">
      <h1>Plate tectonics</h1><h2>Slab pull</h2><h3>Ridge push</h3>
      <h4>Mantle</h4><h5>Notes</h5><h6>Sources</h6>
      <h2>The <strong>larger</strong> force</h2>
      <p>Plates <strong>move</strong> and <b>drift</b>, see <a href="#">the map</a><sup><a href="#">1</a></sup>.</p>
      <p>Press <kbd>N</kbd>.</p>
      <dl><dt>Slab</dt><dd>A sinking plate.</dd></dl>
      <table><thead><tr><th>Force</th></tr></thead><tbody><tr><th>Pull</th></tr></tbody></table>
    </div>`,
  );

  const weights = await page
    .locator(scope)
    .evaluate((element) =>
      Object.fromEntries(
        [...element.querySelectorAll(".typeset *")].map((node, index) => [
          `${index} ${node.tagName.toLowerCase()} ${node.textContent}`,
          getComputedStyle(node).fontWeight,
        ]),
      ),
    );
  for (const [node, weight] of Object.entries(weights)) {
    expect(["400", "510"], node).toContain(weight);
  }
  const emphasis = await page
    .locator(`${scope} :is(h1, h2, h3, strong)`)
    .evaluateAll((nodes) =>
      nodes.map((node) => getComputedStyle(node).fontWeight),
    );
  expect(new Set(emphasis)).toEqual(new Set(["510"]));
});
