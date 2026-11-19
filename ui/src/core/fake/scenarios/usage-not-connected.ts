import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description: "The usage meter is empty until Connect usage.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.usage = { type: "NotConnected" };
  },
} satisfies Scenario;
