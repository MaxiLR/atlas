import type { Scenario } from "../scenario";
import { signIn } from "../world";

export default {
  description:
    "Signed in with plan usage on and no courses; placement of a new course asks six questions before the index.",
  setup(world) {
    signIn(world);
    world.placementQuestions = 6;
  },
} satisfies Scenario;
