import type { Scenario } from "../scenario";
import { addSampleLibrary, connectUsage, iso, signIn } from "../world";

export default {
  description:
    "The plan's limit is reached: a generation stops, and the reset time is known.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    const resetsAt = iso(world, (2 * 60 + 14) * 60_000);
    world.usageShares = [1, 0.74, 0.3];
    connectUsage(world);
    world.fault = {
      type: "Fail",
      error: { type: "PlanLimit", resetsAt },
      retries: 0,
    };
  },
} satisfies Scenario;
