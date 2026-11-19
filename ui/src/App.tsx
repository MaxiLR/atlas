import { useEffect } from "react";

import {
  AtlasStoreProvider,
  failureInStore,
  useAtlas,
  useAtlasStore,
} from "@/app/store";
import { Library } from "@/screens/Library";
import { Placement } from "@/screens/Placement";
import { Reader } from "@/screens/Reader";
import { Settings } from "@/screens/Settings";
import { SignIn } from "@/screens/SignIn";
import { selectOpenCourse } from "@/store";
import { unknownTag } from "@/store/unknown-tag";

/** The screen the store's navigation names, once the session is known. */
function Screen() {
  const { send } = useAtlasStore();
  const { screen, known, course } = useAtlas((state) => ({
    screen: state.navigation.screen,
    known: state.session.session !== null,
    course: selectOpenCourse(state),
  }));

  useEffect(() => {
    send({ type: "GetSession" }).catch(failureInStore);
  }, [send]);

  if (!known) return <p role="status">Opening Atlas…</p>;
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

export function App() {
  return (
    <AtlasStoreProvider>
      <Screen />
    </AtlasStoreProvider>
  );
}
