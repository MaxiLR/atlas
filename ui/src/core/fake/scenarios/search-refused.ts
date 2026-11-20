import type { Scenario } from "../scenario";
import { addSampleLibrary, signIn } from "../world";

export default {
  description:
    "The account's policy refuses web search: sections are written without it and say so.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.searchRefused = true;
  },
} satisfies Scenario;
