import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";
import { Slot } from "radix-ui";

/**
 * Compact application controls: 28 px high with 6 px corners, 32 px for the
 * primary action of a form. Hover and press change fill and brightness, a
 * press scales to 0.97, focus is an accent outline. A toggle that is on
 * (`aria-pressed`) or a trigger whose menu is open (`aria-expanded`) is
 * filled like a hover that stays.
 */
const buttonVariants = cva(
  "group/button inline-flex shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded-sm border border-transparent text-13 leading-none font-medium whitespace-nowrap transition-[background-color,border-color,color,scale] outline-none select-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-focus active:not-disabled:scale-97 disabled:cursor-not-allowed [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5",
  {
    variants: {
      variant: {
        default:
          "border-edge bg-wash text-body hover:not-disabled:bg-wash-strong hover:not-disabled:text-heading aria-expanded:bg-wash-strong aria-expanded:text-heading aria-pressed:border-edge-strong aria-pressed:bg-wash-strong aria-pressed:text-heading disabled:border-transparent disabled:text-muted-foreground",
        primary:
          "bg-primary text-primary-foreground hover:not-disabled:bg-heading disabled:bg-wash-strong disabled:text-muted-foreground",
        ghost:
          "text-muted-foreground hover:not-disabled:bg-wash hover:not-disabled:text-heading aria-expanded:bg-wash aria-expanded:text-heading aria-pressed:bg-wash-strong aria-pressed:text-heading disabled:opacity-50",
      },
      size: {
        default:
          "h-7 px-2.5 has-[>svg:first-child]:pl-2 has-[>svg:last-child]:pr-2",
        lg: "h-8 gap-2 px-3",
        icon: "size-7",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  }) {
  const Comp = asChild ? Slot.Root : "button";

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
