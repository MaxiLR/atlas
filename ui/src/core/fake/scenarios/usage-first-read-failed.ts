import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "Signed in, the first usage reading fails: the meter says why, and Try again reads again.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.usageFailure = "Network";
  },
} satisfies Scenario;
