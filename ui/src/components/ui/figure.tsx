import { useId } from "react";
import type * as React from "react";
import { cn } from "@/lib/utils";

/**
 * The frame every catalog visual and figure shares: a panel with 12 px
 * corners and a hairline edge, no shadow, the visual on the panel face, and
 * a caption bar under a hairline with the figure's number in uppercase mono.
 * Labels drawn inside an SVG take the same mono style.
 */
function Figure({
  label,
  caption,
  className,
  children,
  ...props
}: Omit<React.ComponentProps<"figure">, "children"> & {
  /** The running number, such as "Fig. 1" or "Table 1". */
  label: string;
  caption: React.ReactNode;
  children: React.ReactNode;
}) {
  const captionId = useId();
  return (
    <figure
      data-slot="figure"
      aria-labelledby={captionId}
      className={cn(
        "min-w-0 overflow-hidden rounded-xl border border-edge bg-panel text-body shadow-none",
        className,
      )}
      {...props}
    >
      <div
        data-slot="figure-content"
        className="min-w-0 overflow-x-auto [&_svg]:block [&_svg]:h-auto [&_svg]:w-full [&_svg_text]:fill-muted-foreground [&_svg_text]:stroke-none [&_svg_text]:font-mono [&_svg_text]:text-12 [&_svg_text]:tracking-[0.04em]"
      >
        {children}
      </div>
      <figcaption
        id={captionId}
        data-slot="figure-caption"
        className="flex items-baseline gap-3.5 border-t border-edge px-4 py-2.5 text-13 text-muted-foreground"
      >
        <span
          data-slot="figure-label"
          className="font-mono text-12 tracking-[0.04em] whitespace-nowrap uppercase"
        >
          {label}
        </span>{" "}
        <span className="min-w-0">{caption}</span>
      </figcaption>
    </figure>
  );
}

export { Figure };
