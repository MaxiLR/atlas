import { Button } from "@/components/ui/button";
import type { Activity } from "@/core/generated/Activity";
import { describeError } from "@/core/client";
import type { SectionDisplay } from "@/store";
import { unknownTag } from "@/store/unknown-tag";

function activityText(activity: Activity | null): string {
  if (activity === null) return "Writing this section…";
  switch (activity.type) {
    case "Writing":
      return "Writing this section…";
    case "Searching":
      return "Searching the web…";
    case "Retrying":
      return `Retrying, attempt ${activity.attempt}…`;
    default:
      return unknownTag(activity);
  }
}

/**
 * Where the section stands: opening, being written with Stop, or stopped,
 * failed or refused with Retry. Nothing once a version is shown.
 */
export function SectionStatus({
  display,
  onStop,
  onRetry,
}: {
  display: SectionDisplay | null;
  onStop: () => void;
  onRetry: () => void;
}) {
  if (
    display === null ||
    (display.status === "empty" && display.error === null)
  ) {
    return (
      <p role="status" className="text-13 text-muted-foreground">
        Opening this section…
      </p>
    );
  }
  if (display.status === "streaming") {
    return (
      <div className="flex items-center gap-3 text-13 text-muted-foreground">
        <p role="status">{activityText(display.activity)}</p>
        <Button variant="ghost" onClick={onStop}>
          Stop
        </Button>
      </div>
    );
  }
  if (display.status === "incomplete" || display.error !== null) {
    return (
      <div className="flex flex-col items-start gap-2 rounded-lg border border-edge px-4 py-3 text-13 text-muted-foreground">
        {display.status === "incomplete" && (
          <p role="status" className="text-body">
            This section is incomplete.
          </p>
        )}
        {display.error !== null && (
          <p role="alert">{describeError(display.error)}</p>
        )}
        <Button onClick={onRetry}>Retry</Button>
      </div>
    );
  }
  return null;
}
