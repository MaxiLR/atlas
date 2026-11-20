import { ChevronDownIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { isThemeMode, type ThemeMode } from "@/theme/mode";
import { useTheme } from "@/theme/provider";

const modes: { mode: ThemeMode; name: string }[] = [
  { mode: "light", name: "Light" },
  { mode: "dark", name: "Dark" },
  { mode: "system", name: "System" },
];

/**
 * A ghost control named for the theme mode in use, whose menu picks light,
 * dark or the system's scheme. The choice applies at once and is kept.
 */
export function ThemeMenu() {
  const { mode, setMode } = useTheme();
  const name = modes.find((option) => option.mode === mode)?.name ?? mode;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" aria-label={`Theme: ${name}`}>
          {name}
          <ChevronDownIcon aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup
          value={mode}
          onValueChange={(next) => isThemeMode(next) && setMode(next)}
        >
          {modes.map((option) => (
            <DropdownMenuRadioItem key={option.mode} value={option.mode}>
              {option.name}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
