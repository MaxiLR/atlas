import type { Scenario } from "../scenario";
import { addSampleLibrary } from "../world";

export default {
  description:
    "Signed out because the plan login was revoked or expired; the courses are kept for after signing in again.",
  setup(world) {
    world.session = { type: "SignedOut", reason: { type: "LoginEnded" } };
    addSampleLibrary(world);
  },
} satisfies Scenario;
