import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "The session cannot be read: a notice says why with Retry, and asking again reads it.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.sessionUnreadable = {
      type: "Store",
      message:
        "The secret store failed: secrets.json in the app-data folder is not valid JSON.",
    };
  },
} satisfies Scenario;
