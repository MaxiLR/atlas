import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description: "The plan's limit is reached and nothing says when it resets.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.fault = {
      type: "Fail",
      error: { type: "PlanLimit", resetsAt: null },
      retries: 0,
    };
  },
} satisfies Scenario;
