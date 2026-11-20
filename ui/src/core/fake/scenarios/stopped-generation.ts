import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "The next section generated is stopped after its first block; Retry completes it.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.fault = { type: "Stop", once: true };
  },
} satisfies Scenario;
