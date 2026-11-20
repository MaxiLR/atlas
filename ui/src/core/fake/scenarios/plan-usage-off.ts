import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "Signed in without permission to use the plan: no course can start until it is enabled.",
  setup(world) {
    signIn(world, "Off");
    addSampleLibrary(world);
  },
} satisfies Scenario;
