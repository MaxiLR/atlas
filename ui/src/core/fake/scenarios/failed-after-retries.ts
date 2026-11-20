import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "The network keeps failing: a generation retries three times, then fails with Retry.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.fault = {
      type: "Fail",
      error: { type: "FailedAfterRetries", attempts: 4 },
      retries: 3,
    };
  },
} satisfies Scenario;
