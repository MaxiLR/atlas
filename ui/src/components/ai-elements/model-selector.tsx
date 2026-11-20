// AI Elements: Model Selector (https://ui.shadcn.com/docs/ai-elements/radix/model-selector).
// Picks the model the agent runs. Atlas sets no reasoning effort, so the
// pattern's effort menu is left out.
import type { ComponentProps, ReactNode } from "react";
import { ChevronDownIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";

export interface ModelOption {
  slug: string;
  displayName: string;
}

/**
 * The tooltip of a model's name: the name in full when it is long enough to
 * be cut short at the control's width, else none, so a screen reader does
 * not hear a short name twice.
 */
export function modelTitle(name: string | null): string | undefined {
  return name !== null && name.length > 24 ? name : undefined;
}

/**
 * A ghost control named for the model in use whose menu lists `models` in
 * the order given, with the one in use marked. A long name is cut short on
 * the control and in the menu and given in full to its accessible name and
 * its tooltip. Other props go to the control.
 */
export function ModelSelector({
  models,
  value,
  onValueChange,
  name = models.find((model) => model.slug === value)?.displayName ?? value,
  marker,
  children,
  className,
  ...trigger
}: {
  models: ModelOption[];
  /** The slug in use; null when none is known. */
  value: string | null;
  onValueChange: (slug: string) => void;
  /**
   * The name the control shows: by default the display name of the model in
   * use; null shows the neutral placeholder.
   */
  name?: string | null;
  /** Shown on the control after the name, such as a sign of trouble. */
  marker?: ReactNode;
  /** Shown in the menu after the models. */
  children?: ReactNode;
} & Omit<ComponentProps<typeof Button>, "value" | "name" | "children">) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          aria-label={name === null ? "Model" : `Model: ${name}`}
          title={modelTitle(name)}
          className={cn("max-w-56", className)}
          {...trigger}
        >
          <span className="min-w-0 truncate">{name ?? "Model"}</span>
          {marker}
          <ChevronDownIcon />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="max-w-72">
        {models.length > 0 && (
          <DropdownMenuRadioGroup
            value={value ?? ""}
            onValueChange={onValueChange}
          >
            {models.map((model) => (
              <DropdownMenuRadioItem
                key={model.slug}
                value={model.slug}
                title={modelTitle(model.displayName)}
              >
                <span className="min-w-0 truncate">{model.displayName}</span>
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        )}
        {children}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
