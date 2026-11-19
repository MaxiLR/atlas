// AI Elements: Model Selector (https://ui.shadcn.com/docs/ai-elements/radix/model-selector).
// Picks the model the agent runs. Atlas sets no reasoning effort, so the
// pattern's effort menu is left out.
import { ChevronDownIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

export interface ModelOption {
  slug: string;
  displayName: string;
}

export function ModelSelector({
  models,
  value,
  onValueChange,
}: {
  models: ModelOption[];
  value: string;
  onValueChange: (slug: string) => void;
}) {
  const name =
    models.find((model) => model.slug === value)?.displayName ?? value;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" aria-label={`Model: ${name}`}>
          {name}
          <ChevronDownIcon />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuRadioGroup value={value} onValueChange={onValueChange}>
          {models.map((model) => (
            <DropdownMenuRadioItem key={model.slug} value={model.slug}>
              {model.displayName}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
