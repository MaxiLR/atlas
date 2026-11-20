import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "Offline: stored sections, definitions and side chats stay readable; anything needing a request says offline.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.offline = true;
  },
} satisfies Scenario;
