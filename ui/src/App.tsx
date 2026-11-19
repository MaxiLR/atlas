import { useCallback, useEffect } from "react";

import {
  AtlasStoreProvider,
  failureInStore,
  useAtlas,
  useAtlasStore,
} from "@/app/store";
import { WindowFocus } from "@/app/window-focus";
import { Button } from "@/components/ui/button";
import { describeError } from "@/core/client";
import { Reader } from "@/reader/reader";
import { Library } from "@/screens/library/library";
import { Placement } from "@/screens/Placement";
import { Settings } from "@/screens/Settings";
import { SignIn } from "@/screens/sign-in/sign-in";
import { selectOpenCourse } from "@/store";
import { unknownTag } from "@/store/unknown-tag";

/** The screen the store's navigation names. */
function Screen() {
  const { screen, course } = useAtlas((state) => ({
    screen: state.navigation.screen,
    course: selectOpenCourse(state),
  }));

  switch (screen) {
    case "signIn":
      return <SignIn />;
    case "library":
      return <Library />;
    case "settings":
      return <Settings />;
    case "placement":
      return course === null ? (
        <Library />
      ) : (
        <Placement key={course.id} course={course} />
      );
    case "reader":
      return course === null ? (
        <Library />
      ) : (
        <Reader key={course.id} course={course} />
      );
    default:
      return unknownTag(screen);
  }
}

/**
 * Asks the core for the session, then shows the screen. While the core has
 * not answered, a loading line; when it refused, its reason with Retry above
 * the screen, so the rest of Atlas stays reachable.
 */
function Session() {
  const { send } = useAtlasStore();
  const { known, error } = useAtlas((state) => ({
    known: state.session.session !== null,
    error: state.session.error,
  }));
  const ask = useCallback(() => {
    send({ type: "GetSession" }).catch(failureInStore);
  }, [send]);

  useEffect(ask, [ask]);

  if (!known && error === null) {
    return <p role="status">Opening Atlas…</p>;
  }
  return (
    <>
      {error !== null && (
        <div className="flex items-center gap-2 p-4">
          <p role="alert">{describeError(error)}</p>
          <Button variant="ghost" onClick={ask}>
            Retry
          </Button>
        </div>
      )}
      <Screen />
    </>
  );
}

export function App() {
  return (
    <AtlasStoreProvider>
      <WindowFocus />
      <Session />
    </AtlasStoreProvider>
  );
}
