import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { THEME_STORAGE_KEY } from "./mode";
import { ThemeProvider, ThemeScope, useTheme } from "./provider";

type SystemScheme = {
  set(scheme: "light" | "dark"): void;
  restore(): void;
};

/** A stand-in for the system colour scheme that tests can flip. */
function emulateSystemScheme(initial: "light" | "dark"): SystemScheme {
  let dark = initial === "dark";
  const listeners = new Set<() => void>();
  const original = window.matchMedia;
  window.matchMedia = ((query: string) => ({
    get matches() {
      return query.includes("dark") ? dark : !dark;
    },
    media: query,
    addEventListener: (_: string, listener: () => void) =>
      listeners.add(listener),
    removeEventListener: (_: string, listener: () => void) =>
      listeners.delete(listener),
  })) as unknown as typeof window.matchMedia;
  return {
    set(scheme: "light" | "dark") {
      dark = scheme === "dark";
      act(() => listeners.forEach((listener) => listener()));
    },
    restore() {
      window.matchMedia = original;
    },
  };
}

function ModeSwitch() {
  const { mode, theme, setMode } = useTheme();
  return (
    <div>
      <p>
        Mode {mode}, showing {theme}
      </p>
      {(["light", "dark", "system"] as const).map((next) => (
        <button key={next} type="button" onClick={() => setMode(next)}>
          Use {next}
        </button>
      ))}
    </div>
  );
}

const pageTheme = () => document.documentElement.dataset.theme;

describe("ThemeProvider", () => {
  let system: SystemScheme;

  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
    system = emulateSystemScheme("dark");
  });

  afterEach(() => system.restore());

  it("follows the system by default", () => {
    render(
      <ThemeProvider>
        <ModeSwitch />
      </ThemeProvider>,
    );
    expect(screen.getByText("Mode system, showing dark")).toBeInTheDocument();
    expect(pageTheme()).toBe("dark");

    system.set("light");
    expect(screen.getByText("Mode system, showing light")).toBeInTheDocument();
    expect(pageTheme()).toBe("light");
  });

  it("persists a chosen mode and stops following the system", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <ModeSwitch />
      </ThemeProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Use light" }));
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
    expect(pageTheme()).toBe("light");

    system.set("dark");
    expect(pageTheme()).toBe("light");
  });

  it("starts from the stored mode", () => {
    localStorage.setItem(THEME_STORAGE_KEY, "light");
    render(
      <ThemeProvider>
        <ModeSwitch />
      </ThemeProvider>,
    );
    expect(screen.getByText("Mode light, showing light")).toBeInTheDocument();
    expect(pageTheme()).toBe("light");
  });

  it("ignores a stored value that is not a mode", () => {
    localStorage.setItem(THEME_STORAGE_KEY, "sepia");
    render(
      <ThemeProvider>
        <ModeSwitch />
      </ThemeProvider>,
    );
    expect(screen.getByText("Mode system, showing dark")).toBeInTheDocument();
  });
});

describe("ThemeScope", () => {
  it("forces a theme on its subtree whatever the page shows", () => {
    const system = emulateSystemScheme("dark");
    render(
      <ThemeProvider>
        <ThemeScope theme="light">
          <p>Exported page</p>
        </ThemeScope>
      </ThemeProvider>,
    );
    expect(pageTheme()).toBe("dark");
    expect(
      screen.getByText("Exported page").closest("[data-theme]"),
    ).toHaveAttribute("data-theme", "light");
    system.restore();
  });
});
