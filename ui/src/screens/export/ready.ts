import { useEffect, type RefObject } from "react";

/**
 * How the export route tells the shell it can print. The document root
 * carries `data-export-state`: `ready` once every visual is drawn and every
 * font loaded, `failed` when the course could not be read. Each change is
 * also dispatched on `window` as this event, its state in `detail`.
 */
export const EXPORT_STATE_EVENT = "atlas:export-state";

export type ExportState = "ready" | "failed";

/** A visual still drawing shows a skeleton in its place. */
const PENDING = '[data-slot="skeleton"]';

/** How long to wait for a change before looking again. */
const LOOK_AGAIN_MS = 250;

function signal(state: ExportState) {
  document.documentElement.dataset.exportState = state;
  window.dispatchEvent(new CustomEvent(EXPORT_STATE_EVENT, { detail: state }));
}

/** Resolves at the next change under `root`, or after a short while. */
function changeIn(root: HTMLElement): Promise<void> {
  const { promise, resolve } = Promise.withResolvers<void>();
  const observer = new MutationObserver(() => resolve());
  observer.observe(root, {
    subtree: true,
    childList: true,
    attributes: true,
    characterData: true,
  });
  const timer = setTimeout(resolve, LOOK_AGAIN_MS);
  return promise.finally(() => {
    observer.disconnect();
    clearTimeout(timer);
  });
}

/**
 * Waits until nothing under `root` is still drawing and every font the page
 * uses has loaded. False when `watch` was stopped first.
 */
async function settled(
  root: HTMLElement,
  watch: { stopped: boolean },
): Promise<boolean> {
  for (;;) {
    if (watch.stopped) return false;
    if (root.querySelector(PENDING) !== null) {
      await changeIn(root);
      continue;
    }
    // Laying the page out starts loading the fonts it uses, KaTeX's among
    // them, so the font set knows what it is waiting for.
    root.getBoundingClientRect();
    const fonts = document.fonts as FontFaceSet | undefined;
    if (fonts !== undefined && fonts.status !== "loaded") {
      await fonts.ready;
      continue;
    }
    return !watch.stopped;
  }
}

/**
 * Signals the export's state: ready once the loaded document under `root`
 * has settled, failed at once. Nothing is signalled while it loads, and the
 * state is cleared when the route leaves.
 */
export function useExportSignal(
  root: RefObject<HTMLElement | null>,
  phase: "loading" | ExportState,
) {
  useEffect(() => {
    if (phase === "loading") return;
    const clear = () => {
      delete document.documentElement.dataset.exportState;
    };
    if (phase === "failed") {
      signal("failed");
      return clear;
    }
    const element = root.current;
    if (element === null) return;
    const watch = { stopped: false };
    void settled(element, watch).then((done) => {
      if (done) signal("ready");
    });
    return () => {
      watch.stopped = true;
      clear();
    };
  }, [root, phase]);
}
