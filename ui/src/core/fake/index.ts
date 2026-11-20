/**
 * The fake core: an in-memory `CoreClient` that speaks the final protocol,
 * in a named scenario, for tests and Vite's `fake` mode. `main.tsx` imports
 * it only in that mode, so the production bundle never contains it.
 */
export {
  createManualClock,
  systemClock,
  type FakeClock,
  type ManualClock,
} from "./clock";
export { createFakeCore, type FakeCore, type FakeCoreOptions } from "./core";
export { scenarioNames, type Scenario } from "./scenario";
export type { FakeWorld } from "./world";
