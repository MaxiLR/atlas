import { useEffect, useSyncExternalStore, type RefObject } from "react";

/**
 * Below 1024 px the rail folds to its line, and the titles and the side
 * panel open over the reading column.
 */
const NARROW = "(width < 64rem)";

function subscribeNarrow(changed: () => void) {
  const query = window.matchMedia?.(NARROW);
  query?.addEventListener("change", changed);
  return () => query?.removeEventListener("change", changed);
}

/** Whether the window is below 1024 px. */
export function useNarrow(): boolean {
  return useSyncExternalStore(
    subscribeNarrow,
    () => window.matchMedia?.(NARROW).matches ?? false,
  );
}

const FOCUSABLE =
  "a[href], button, input, textarea, select, [tabindex], [contenteditable]";

/** The elements of `container` that Tab stops at, in order. */
function tabStops(container: HTMLElement): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (element) =>
      element.tabIndex >= 0 &&
      !element.matches(":disabled") &&
      element.closest("[inert]") === null &&
      (element.checkVisibility?.() ?? true),
  );
}

/**
 * While `held`, Tab and Shift+Tab go round the stops of `container` instead
 * of leaving it: an overlay whose surroundings are inert keeps focus.
 */
export function useFocusLoop(
  container: RefObject<HTMLElement | null>,
  held: boolean,
) {
  useEffect(() => {
    const element = container.current;
    if (!held || element === null) return;
    const loop = (event: KeyboardEvent) => {
      if (event.key !== "Tab" || event.defaultPrevented) return;
      const stops = tabStops(element);
      const first = stops[0];
      const last = stops.at(-1);
      if (first === undefined || last === undefined) return;
      const from = document.activeElement;
      // Focus lost to the page comes back in; a layer of its own, such as a
      // menu opened from the overlay, handles Tab itself.
      const outside = from === null || from === document.body;
      if (!outside && !element.contains(from)) return;
      if (event.shiftKey && (outside || from === first)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (outside || from === last)) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", loop);
    return () => document.removeEventListener("keydown", loop);
  }, [container, held]);
}
