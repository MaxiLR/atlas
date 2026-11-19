import type { Page } from "@playwright/test";

export interface AppOptions {
  /** A scenario of the fake core, by its file name; `default` when left out. */
  scenario?: string;
  /** The window width in px; the project's when left out. */
  width?: number;
  /** The system colour scheme; the project's when left out. */
  colorScheme?: "light" | "dark" | "no-preference";
}

/** Opens the app on the fake core in `scenario`, at a width and colour scheme. */
export async function openApp(page: Page, options: AppOptions = {}) {
  const { scenario = "default", width, colorScheme } = options;
  if (width !== undefined) {
    await page.setViewportSize({
      width,
      height: page.viewportSize()?.height ?? 900,
    });
  }
  if (colorScheme !== undefined) {
    await page.emulateMedia({ colorScheme });
  }
  await page.goto(`/?scenario=${encodeURIComponent(scenario)}`);
}
