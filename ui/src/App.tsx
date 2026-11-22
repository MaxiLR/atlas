import { useEffect } from "react";

import { Notices } from "@/app/notices";
import {
  AtlasStoreProvider,
  failureInStore,
  useAtlas,
  useAtlasStore,
} from "@/app/store";
import { WindowFocus } from "@/app/window-focus";
import { Reader } from "@/reader/reader";
import { ExportScreen } from "@/screens/export/export-screen";
import { useExportedCourse } from "@/screens/export/route";
import { Library } from "@/screens/library/library";
import { Placement } from "@/screens/placement/placement";
import { Settings } from "@/screens/settings/settings";
import { SignIn } from "@/screens/sign-in/sign-in";
import { selectOpenCourse } from "@/store";
import { unknownTag } from "@/store/unknown-tag";
import { ThemeProvider } from "@/theme/provider";

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
 * not answered, a loading line; when it refused, the screen, so the rest of
 * Atlas stays reachable, and a notice says why with Retry.
 */
function Session() {
  const { send } = useAtlasStore();
  const opening = useAtlas(
    (state) => state.session.session === null && state.session.error === null,
  );

  useEffect(() => {
    send({ type: "GetSession" }).catch(failureInStore);
  }, [send]);

  return opening ? <p role="status">Opening Atlas…</p> : <Screen />;
}

/**
 * The app, or at the export route the course as it is printed: the shell
 * loads that route on its own to make a PDF, without session or screens.
 */
export function App() {
  const exported = useExportedCourse();
  return (
    <ThemeProvider>
      <AtlasStoreProvider>
        {exported === null ? (
          <>
            <WindowFocus />
            <Session />
            <Notices />
          </>
        ) : (
          <ExportScreen key={exported} courseId={exported} />
        )}
      </AtlasStoreProvider>
    </ThemeProvider>
  );
}
