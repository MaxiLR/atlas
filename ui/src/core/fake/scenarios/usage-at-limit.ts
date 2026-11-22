import type { Scenario } from "../scenario";
import { addSampleLibrary, readUsage, signIn } from "../world";

export default {
  description: "The 5-hour window is at its limit.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.usageShares = [1, 0.82, 0.3];
    readUsage(world);
  },
} satisfies Scenario;
