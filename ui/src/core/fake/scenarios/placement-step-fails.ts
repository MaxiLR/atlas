import type { Scenario } from "../scenario";
import { signIn } from "../world";

export default {
  description:
    "Signed in with plan usage on and no courses; building a new course's index fails once after four questions, and Retry builds it.",
  setup(world) {
    signIn(world);
    world.placementFailure = {
      step: "BuildingIndex",
      error: { type: "FailedAfterRetries", attempts: 4 },
    };
  },
} satisfies Scenario;
