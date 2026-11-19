import type { Scenario } from "../scenario";
import { addSampleLibrary, connectUsage, signIn } from "../world";

export default {
  description: "The 5-hour window is at its limit.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.usageShares = [1, 0.82, 0.3];
    connectUsage(world);
  },
} satisfies Scenario;
