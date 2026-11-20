/** The fake core's time: what it stamps records with, and its timed readings. */
export interface FakeClock {
  now(): Date;
  /** Resolves once `ms` have passed on this clock. */
  sleep(ms: number): Promise<void>;
}

/** The wall clock, for the app in Vite mode `fake`. */
export const systemClock: FakeClock = {
  now: () => new Date(),
  sleep(ms) {
    const { promise, resolve } = Promise.withResolvers<void>();
    setTimeout(resolve, ms);
    return promise;
  },
};

export interface ManualClock extends FakeClock {
  /** Moves the clock on, waking every sleep that has run out, in order. */
  advance(ms: number): void;
}

/** A clock that moves only when a test advances it. */
export function createManualClock(start: string): ManualClock {
  let now = Date.parse(start);
  let sleepers: { until: number; wake: () => void }[] = [];
  return {
    now: () => new Date(now),
    sleep(ms) {
      const { promise, resolve } = Promise.withResolvers<void>();
      sleepers.push({ until: now + ms, wake: resolve });
      return promise;
    },
    advance(ms) {
      now += ms;
      const due = sleepers
        .filter((sleeper) => sleeper.until <= now)
        .sort((a, b) => a.until - b.until);
      sleepers = sleepers.filter((sleeper) => sleeper.until > now);
      for (const sleeper of due) sleeper.wake();
    },
  };
}
