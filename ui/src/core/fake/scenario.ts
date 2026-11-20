import type { FakeWorld } from "./world";

/**
 * A named state of the fake core, one file in `scenarios/` each: the file's
 * name is the scenario's name, and its default export builds the world.
 */
export interface Scenario {
  /** What the app shows in it, in a sentence. */
  description: string;
  setup(world: FakeWorld): void;
}

const modules = import.meta.glob<Scenario>(
  ["./scenarios/*.ts", "!./scenarios/*.test.ts"],
  { eager: true, import: "default" },
);

/** Every scenario found in `scenarios/`, by name. */
export const scenarios: Record<string, Scenario> = Object.fromEntries(
  Object.entries(modules).map(([path, scenario]) => [
    path.slice("./scenarios/".length, -".ts".length),
    scenario,
  ]),
);

export const scenarioNames: string[] = Object.keys(scenarios).sort();
