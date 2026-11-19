import * as React from "react";
import { cn } from "@/lib/utils";

/** A multi-line field, drawn like `Input`. */
function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex field-sizing-content min-h-16 w-full rounded-lg border border-edge-strong bg-panel px-3 py-2 text-15 text-heading transition-[border-color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-focus focus-visible:ring-3 focus-visible:ring-atlas-accent-wash disabled:cursor-not-allowed disabled:bg-transparent disabled:text-muted-foreground aria-invalid:border-destructive",
        className,
      )}
      {...props}
    />
  );
}

export { Textarea };
