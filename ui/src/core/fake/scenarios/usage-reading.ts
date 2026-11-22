import type { Scenario } from "../scenario";
import { addSampleLibrary, readUsage, signIn } from "../world";

export default {
  description:
    "Usage read: the 5-hour and weekly windows, and a monthly one in the popover.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    readUsage(world);
  },
} satisfies Scenario;
