import {
  createContext,
  useCallback,
  useContext,
  useLayoutEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";

import type { Scheme } from "./generate";
import {
  isThemeMode,
  SYSTEM_DARK_QUERY,
  THEME_STORAGE_KEY,
  type ThemeMode,
} from "./mode";

type ThemeState = {
  /** The chosen mode. */
  mode: ThemeMode;
  /** The theme the page shows. */
  theme: Scheme;
  setMode: (mode: ThemeMode) => void;
};

const ThemeContext = createContext<ThemeState | null>(null);

function readStoredMode(): ThemeMode {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    return isThemeMode(stored) ? stored : "system";
  } catch {
    return "system";
  }
}

// A host without media queries (jsdom) reports a light system that never
// changes.
function subscribeToSystem(onChange: () => void) {
  const query = window.matchMedia?.(SYSTEM_DARK_QUERY);
  query?.addEventListener("change", onChange);
  return () => query?.removeEventListener("change", onChange);
}

function systemScheme(): Scheme {
  return window.matchMedia?.(SYSTEM_DARK_QUERY).matches ? "dark" : "light";
}

/** Swaps the page's theme without letting transitions animate the swap. */
function showTheme(theme: Scheme) {
  const root = document.documentElement;
  if (root.dataset.theme === theme) return;
  const pause = document.createElement("style");
  pause.textContent = "*,*::before,*::after{transition:none!important}";
  document.head.append(pause);
  root.dataset.theme = theme;
  // Apply the new styles while transitions are off, then allow them again.
  getComputedStyle(root).getPropertyValue("color-scheme");
  setTimeout(() => pause.remove());
}

/**
 * Holds the theme mode: light, dark or system (the default), persisted in
 * local storage, following system changes live. The document head already
 * shows the right theme before this mounts (see `themeBootScript`).
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const [mode, setStoredMode] = useState(readStoredMode);
  const system = useSyncExternalStore(subscribeToSystem, systemScheme);
  const theme = mode === "system" ? system : mode;

  useLayoutEffect(() => showTheme(theme), [theme]);

  const setMode = useCallback((next: ThemeMode) => {
    try {
      localStorage.setItem(THEME_STORAGE_KEY, next);
    } catch {
      // Without storage the choice lasts until the window closes.
    }
    setStoredMode(next);
  }, []);

  const state = useMemo(
    () => ({ mode, theme, setMode }),
    [mode, theme, setMode],
  );
  return <ThemeContext value={state}>{children}</ThemeContext>;
}

export function useTheme(): ThemeState {
  const state = useContext(ThemeContext);
  if (!state) throw new Error("useTheme must be used within a ThemeProvider");
  return state;
}

/**
 * Shows its subtree in one theme whatever the page shows, as the PDF export
 * does with the light theme.
 */
export function ThemeScope({
  theme,
  className,
  children,
}: {
  theme: Scheme;
  className?: string;
  children: ReactNode;
}) {
  return (
    <div data-theme={theme} className={className}>
      {children}
    </div>
  );
}
