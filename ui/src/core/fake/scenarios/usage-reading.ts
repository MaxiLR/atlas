import type { Scenario } from "../scenario";
import { addSampleLibrary, connectUsage, signIn } from "../world";

export default {
  description:
    "Usage connected: the 5-hour and weekly windows, and a monthly one in the popover.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    connectUsage(world);
  },
} satisfies Scenario;
