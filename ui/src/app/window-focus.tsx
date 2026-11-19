import { useEffect } from "react";

import { failureInStore, useAtlasStore } from "@/app/store";

/**
 * Tells the core whether the window has focus: once when the interface
 * starts, then each time the window gains or loses it. The core reads usage
 * every five minutes only while it does.
 */
export function WindowFocus() {
  const { send } = useAtlasStore();

  useEffect(() => {
    const tell = (focused: boolean) => {
      send({ type: "SetWindowFocused", focused }).catch(failureInStore);
    };
    const gained = () => tell(true);
    const lost = () => tell(false);
    tell(document.hasFocus());
    window.addEventListener("focus", gained);
    window.addEventListener("blur", lost);
    return () => {
      window.removeEventListener("focus", gained);
      window.removeEventListener("blur", lost);
    };
  }, [send]);

  return null;
}
