import type { Scenario } from "../scenario";
import { addSampleLibrary, connectUsage, signIn } from "../world";

export default {
  description:
    "The latest usage reading failed: the last one stays, marked stale with the reason.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    connectUsage(world);
    world.usageFailure = "Network";
    if (world.usage.type === "Connected") {
      world.usage.reading.stale = "Network";
    }
  },
} satisfies Scenario;
