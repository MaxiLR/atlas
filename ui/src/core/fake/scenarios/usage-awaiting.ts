import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "Signed in, the first usage reading never lands: the meter waits for it with its rows in place.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.usage = { type: "AwaitingReading" };
    world.firstReadingHeld = true;
  },
} satisfies Scenario;
