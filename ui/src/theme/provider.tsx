import type { ReactNode } from "react";

export type ThemeMode = "light" | "dark" | "system";

export function ThemeProvider({ children }: { children: ReactNode }) {
  return children;
}

export function useTheme(): {
  mode: ThemeMode;
  theme: "light" | "dark";
  setMode: (mode: ThemeMode) => void;
} {
  throw new Error("useTheme is not implemented");
}

export function ThemeScope({
  children,
}: {
  theme: "light" | "dark";
  children: ReactNode;
}) {
  return children;
}
