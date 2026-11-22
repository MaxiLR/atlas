import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeAll, describe, expect, it } from "vitest";

import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogTitle,
} from "./alert-dialog";
import { Dialog, DialogContent, DialogTitle } from "./dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "./dropdown-menu";
import { HoverCard, HoverCardContent, HoverCardTrigger } from "./hover-card";
import { Popover, PopoverContent, PopoverTrigger } from "./popover";
import { Sheet, SheetContent, SheetTitle } from "./sheet";
import { Notice } from "./sonner";

beforeAll(() => {
  // Radix measures floating content; jsdom has no layout to observe.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

const cases: [string, ReactNode, string[]][] = [
  [
    "a popover",
    <Popover open>
      <PopoverTrigger>Define</PopoverTrigger>
      <PopoverContent>Lithosphere, the rigid outer shell</PopoverContent>
    </Popover>,
    ["Lithosphere, the rigid outer shell"],
  ],
  [
    "a menu and its submenu",
    <DropdownMenu open>
      <DropdownMenuTrigger>Course</DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem>Rename course</DropdownMenuItem>
        <DropdownMenuSub open>
          <DropdownMenuSubTrigger>Export</DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            <DropdownMenuItem>As PDF</DropdownMenuItem>
          </DropdownMenuSubContent>
        </DropdownMenuSub>
      </DropdownMenuContent>
    </DropdownMenu>,
    ["Rename course", "As PDF"],
  ],
  [
    "a hover card",
    <HoverCard open>
      <HoverCardTrigger>Source</HoverCardTrigger>
      <HoverCardContent>Plate tectonics, Wikipedia</HoverCardContent>
    </HoverCard>,
    ["Plate tectonics, Wikipedia"],
  ],
  [
    "a dialog",
    <Dialog open>
      <DialogContent aria-describedby={undefined}>
        <DialogTitle>Rename course</DialogTitle>
      </DialogContent>
    </Dialog>,
    ["Rename course"],
  ],
  [
    "an alert dialog",
    <AlertDialog open>
      <AlertDialogContent aria-describedby={undefined}>
        <AlertDialogTitle>Delete course</AlertDialogTitle>
      </AlertDialogContent>
    </AlertDialog>,
    ["Delete course"],
  ],
  [
    "a sheet",
    <Sheet open>
      <SheetContent aria-describedby={undefined}>
        <SheetTitle>Side chats</SheetTitle>
      </SheetContent>
    </Sheet>,
    ["Side chats"],
  ],
  [
    "a notice",
    <Notice
      kind="error"
      title="Session could not be read"
      description="secrets.json is not valid JSON."
      action={{ label: "Retry", onClick() {} }}
      onDismiss={() => {}}
    />,
    ["Session could not be read", "secrets.json is not valid JSON."],
  ],
];

describe("floating surfaces", () => {
  it.each(cases)(
    "%s re-derives its tokens as a raised surface",
    (_, ui, texts) => {
      render(ui);
      for (const text of texts) {
        // The nearest surface decides which tokens the text is painted with.
        expect(
          screen.getByText(text).closest("[data-surface]"),
          text,
        ).toHaveAttribute("data-surface", "raised");
      }
    },
  );
});
