"use client";

import * as React from "react";
import { cn } from "@/lib/utils";
import { Progress as ProgressPrimitive } from "radix-ui";

/** A 4 px bar: a hairline track filled in the body colour, `value` out of `max`. */
function Progress({
  className,
  value,
  max = 100,
  ...props
}: React.ComponentProps<typeof ProgressPrimitive.Root>) {
  const share = max > 0 ? ((value ?? 0) / max) * 100 : 0;
  return (
    <ProgressPrimitive.Root
      data-slot="progress"
      value={value}
      max={max}
      className={cn(
        "relative flex h-1 w-full items-center overflow-hidden rounded-[2px] bg-edge-strong",
        className,
      )}
      {...props}
    >
      <ProgressPrimitive.Indicator
        data-slot="progress-indicator"
        className="size-full flex-1 rounded-[2px] bg-body transition-transform"
        style={{ transform: `translateX(-${100 - share}%)` }}
      />
    </ProgressPrimitive.Root>
  );
}

export { Progress };
