import { useSyncExternalStore } from "react";

/**
 * The export route of a course is the app's page with the hash
 * `#/export/<course id>`: the shell loads it in a webview it never shows and
 * prints it once the route signals ready (see `ready.ts`).
 */
const EXPORT_ROUTE = /^#\/export\/([^/?#]+)$/;

/** The course a location hash exports; null for any other hash. */
export function exportedCourseOf(hash: string): string | null {
  const id = EXPORT_ROUTE.exec(hash)?.[1];
  return id === undefined ? null : decodeURIComponent(id);
}

function subscribeToHash(onChange: () => void) {
  window.addEventListener("hashchange", onChange);
  return () => window.removeEventListener("hashchange", onChange);
}

/** The course the window's location exports, following hash changes. */
export function useExportedCourse(): string | null {
  return useSyncExternalStore(subscribeToHash, () =>
    exportedCourseOf(window.location.hash),
  );
}
