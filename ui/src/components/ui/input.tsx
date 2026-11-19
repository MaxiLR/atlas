import * as React from "react";
import { cn } from "@/lib/utils";

/** A text field: 10 px corners, a hairline edge, the accent edge on focus. */
function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        "h-9 w-full min-w-0 rounded-lg border border-edge-strong bg-panel px-3 text-15 leading-none text-heading transition-[border-color,box-shadow] outline-none file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-13 file:font-medium file:text-heading placeholder:text-muted-foreground focus-visible:border-focus focus-visible:ring-3 focus-visible:ring-atlas-accent-wash disabled:cursor-not-allowed disabled:bg-transparent disabled:text-muted-foreground aria-invalid:border-destructive",
        className,
      )}
      {...props}
    />
  );
}

export { Input };
