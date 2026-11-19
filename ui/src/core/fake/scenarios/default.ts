import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "Signed in with plan usage on; Plate tectonics part read and a course left in placement.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
  },
} satisfies Scenario;
