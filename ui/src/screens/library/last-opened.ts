const time = new Intl.DateTimeFormat("en-GB", {
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});
const thisYear = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "short",
});
const otherYear = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "short",
  year: "numeric",
});

/** Calendar days from `earlier` to `later`, in local time. */
function daysBetween(earlier: Date, later: Date): number {
  const start = new Date(earlier).setHours(0, 0, 0, 0);
  const end = new Date(later).setHours(0, 0, 0, 0);
  // Rounded: a day across a clock change is 23 or 25 hours.
  return Math.round((end - start) / 86_400_000);
}

/**
 * When a course was last opened, as the library row says it: today with the
 * time, yesterday, then the day and month, with the year when it is not this
 * one.
 */
export function lastOpened(at: Date, now: Date): string {
  const days = daysBetween(at, now);
  if (days <= 0) return `Today, ${time.format(at)}`;
  if (days === 1) return "Yesterday";
  return at.getFullYear() === now.getFullYear()
    ? thisYear.format(at)
    : otherYear.format(at);
}
