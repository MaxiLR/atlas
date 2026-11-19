import * as React from "react";
import { cn } from "@/lib/utils";
import { RadioGroup as RadioGroupPrimitive } from "radix-ui";

function RadioGroup({
  className,
  ...props
}: React.ComponentProps<typeof RadioGroupPrimitive.Root>) {
  return (
    <RadioGroupPrimitive.Root
      data-slot="radio-group"
      className={cn("grid w-full gap-2", className)}
      {...props}
    />
  );
}

function RadioGroupItem({
  className,
  ...props
}: React.ComponentProps<typeof RadioGroupPrimitive.Item>) {
  return (
    <RadioGroupPrimitive.Item
      data-slot="radio-group-item"
      className={cn(
        "group/radio-group-item peer relative flex aspect-square size-4 shrink-0 cursor-pointer rounded-full border border-input outline-none after:absolute after:-inset-x-3 after:-inset-y-2 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive data-checked:border-[5px] data-checked:border-atlas-accent",
        className,
      )}
      {...props}
    />
  );
}

export { RadioGroup, RadioGroupItem };
