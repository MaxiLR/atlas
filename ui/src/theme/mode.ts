/** Light, dark, or whatever the system shows; the default follows the system. */
export type ThemeMode = "light" | "dark" | "system";

export const THEME_STORAGE_KEY = "atlas-theme";
export const SYSTEM_DARK_QUERY = "(prefers-color-scheme: dark)";

export function isThemeMode(value: unknown): value is ThemeMode {
  return value === "light" || value === "dark" || value === "system";
}

/**
 * Runs inline in the document head, before the first paint and before the
 * interface bundle, so the page never shows the wrong theme. It must match
 * what `ThemeProvider` resolves.
 */
export const themeBootScript = `(() => {
  let mode = null;
  try {
    mode = localStorage.getItem(${JSON.stringify(THEME_STORAGE_KEY)});
  } catch {}
  document.documentElement.dataset.theme =
    mode === "light" || mode === "dark"
      ? mode
      : matchMedia(${JSON.stringify(SYSTEM_DARK_QUERY)}).matches
        ? "dark"
        : "light";
})();`;
