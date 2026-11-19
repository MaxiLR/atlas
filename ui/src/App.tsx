import { AtlasStoreProvider, useAtlas } from "@/app/store";
import { Library } from "@/screens/Library";
import { Reader } from "@/screens/Reader";
import { selectOpenCourse } from "@/store";

function Screen() {
  const { screen, course } = useAtlas((state) => ({
    screen: state.navigation.screen,
    course: selectOpenCourse(state),
  }));

  return screen === "reader" && course !== null ? (
    <Reader key={course.id} course={course} />
  ) : (
    <Library />
  );
}

export function App() {
  return (
    <AtlasStoreProvider>
      <Screen />
    </AtlasStoreProvider>
  );
}
