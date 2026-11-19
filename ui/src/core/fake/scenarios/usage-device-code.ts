import type { Scenario } from "../scenario";
import { addSampleLibrary, iso, signIn } from "../world";

export default {
  description: "Connecting usage: the device code to enter on OpenAI's page.",
  setup(world) {
    signIn(world);
    addSampleLibrary(world);
    world.usage = {
      type: "Connecting",
      userCode: "QWRT-7KPD",
      verificationUrl: "https://auth.openai.com/codex/device",
      expiresAt: iso(world, 15 * 60_000),
    };
  },
} satisfies Scenario;
