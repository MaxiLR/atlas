import { GaugeIcon } from "lucide-react";
import { useEffect } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import type { UsageState } from "@/core/generated/UsageState";
import type { UsageWindow } from "@/core/generated/UsageWindow";
import { cn } from "@/lib/utils";
import { selectUsage } from "@/store";
import { unknownTag } from "@/store/unknown-tag";

function windowName(window: UsageWindow): string {
  switch (window.name) {
    case "5h":
      return "5-hour";
    case "7d":
      return "Weekly";
    default:
      return window.name;
  }
}

/** What the meter says in each state; `folded` keeps only Connect usage. */
function Reading({
  usage,
  folded,
  onConnect,
  onCancel,
}: {
  usage: UsageState;
  folded: boolean;
  onConnect: () => void;
  onCancel: () => void;
}) {
  const hiddenFolded = folded ? "max-lg:hidden" : undefined;
  switch (usage.type) {
    case "NotConnected":
      return (
        <>
          <p className={hiddenFolded}>See how much of your plan is left.</p>
          <Button
            aria-label="Connect usage"
            className={cn(
              "self-start",
              folded && "max-lg:size-7 max-lg:self-center max-lg:p-0",
            )}
            onClick={onConnect}
          >
            <GaugeIcon aria-hidden />
            <span className={hiddenFolded}>Connect usage</span>
          </Button>
        </>
      );
    case "Connecting":
      return (
        <div className={cn("flex flex-col gap-2", hiddenFolded)}>
          <p>
            Enter{" "}
            <span className="font-medium text-heading tabular-nums">
              {usage.userCode}
            </span>{" "}
            on the page that opened in your browser.
          </p>
          <Button variant="ghost" className="self-start" onClick={onCancel}>
            Cancel
          </Button>
        </div>
      );
    case "Connected":
      return (
        <div className={cn("flex flex-col gap-2", hiddenFolded)}>
          <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
            {usage.reading.windows.map((window) => (
              <div key={window.name} className="contents">
                <dt className="font-medium text-body">{windowName(window)}</dt>
                <dd className="tabular-nums">
                  {Math.round(window.used * 100)}% used
                </dd>
              </div>
            ))}
          </dl>
          {usage.reading.stale !== null && (
            <p>The latest reading failed, so these figures are older.</p>
          )}
        </div>
      );
    case "FirstReadFailed":
      return (
        <div className={cn("flex flex-col gap-2", hiddenFolded)}>
          <p>Usage could not be read.</p>
          <Button className="self-start" onClick={onConnect}>
            Try again
          </Button>
        </div>
      );
    case "SignedOut":
      return <p className={hiddenFolded}>Sign in to see your plan usage.</p>;
    default:
      return unknownTag(usage);
  }
}

/**
 * The usage meter at the foot of the rail. Not connected, it offers Connect
 * usage; folded below 1024 px, only that action shows, as an icon.
 */
export function UsageMeter({ folded }: { folded: boolean }) {
  const { send } = useAtlasStore();
  const usage = useAtlas(selectUsage);

  useEffect(() => {
    send({ type: "GetUsage" }).catch(failureInStore);
  }, [send]);

  if (usage === null) return null;
  return (
    <section
      aria-label="Plan usage"
      className={cn(
        "flex shrink-0 flex-col gap-2 border-t border-edge px-5 pt-3 pb-4 text-12 text-muted-foreground",
        folded && "max-lg:items-center max-lg:px-0",
      )}
    >
      <Reading
        usage={usage}
        folded={folded}
        onConnect={() => send({ type: "ConnectUsage" }).catch(failureInStore)}
        onCancel={() =>
          send({ type: "CancelConnectUsage" }).catch(failureInStore)
        }
      />
    </section>
  );
}
