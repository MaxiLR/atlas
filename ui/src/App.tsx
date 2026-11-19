import { useState } from "react";

import type { CourseView } from "@/core/generated/CourseView";
import { Library } from "@/screens/Library";
import { Reader } from "@/screens/Reader";

type Screen = { name: "library" } | { name: "reader"; course: CourseView };

export function App() {
  const [screen, setScreen] = useState<Screen>({ name: "library" });

  return screen.name === "library" ? (
    <Library onOpen={(course) => setScreen({ name: "reader", course })} />
  ) : (
    <Reader
      key={screen.course.id}
      course={screen.course}
      onBack={() => setScreen({ name: "library" })}
    />
  );
}
