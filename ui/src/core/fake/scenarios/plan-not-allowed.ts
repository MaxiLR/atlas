import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "Signed in, but the account or workspace may not use the plan: explained once, no sign-in loop.",
  setup(world) {
    signIn(world, "NotAllowed");
    addSampleLibrary(world);
  },
} satisfies Scenario;
