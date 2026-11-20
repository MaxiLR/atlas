import { cn } from "@/lib/utils";

/** A placeholder of the size the content will take; hidden from assistive technology. */
function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      aria-hidden="true"
      className={cn(
        "animate-pulse rounded-sm bg-wash motion-reduce:animate-none",
        className,
      )}
      {...props}
    />
  );
}

export { Skeleton };
