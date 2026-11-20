/**
 * A pace for the fake core that lets a test release streamed deltas one at a
 * time: each `step()` lets one waiting (or future) `wait()` through.
 */
export function createGate() {
  let permits = 0;
  const waiting: (() => void)[] = [];
  return {
    wait(): Promise<void> {
      if (permits > 0) {
        permits -= 1;
        return Promise.resolve();
      }
      const { promise, resolve } = Promise.withResolvers<void>();
      waiting.push(resolve);
      return promise;
    },
    step() {
      const next = waiting.shift();
      if (next) next();
      else permits += 1;
    },
  };
}
