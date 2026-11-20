import type { Scenario } from "../scenario";
import { signIn } from "../world";

export default {
  description:
    "Signed in with plan usage on and no courses: home shows only the topic field.",
  setup(world) {
    signIn(world);
  },
} satisfies Scenario;
