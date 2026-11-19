import { render, screen, waitFor } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, beforeAll, describe, expect, it } from "vitest";

import {
  applyStyles,
  clearStyles,
  endAnimation,
  literalColorsIn,
  sameColor,
  schemes,
  token,
  tokenOf,
} from "@/test/styles";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "./alert-dialog";
import { Button } from "./button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "./dialog";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "./dropdown-menu";
import { HoverCard, HoverCardContent, HoverCardTrigger } from "./hover-card";
import { Input } from "./input";
import { Popover, PopoverContent, PopoverTrigger } from "./popover";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "./sheet";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./tooltip";

beforeAll(() => {
  // Radix measures floating content; jsdom has no layout to observe.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

afterEach(clearStyles);

/** One layer of a computed `box-shadow`. */
type ShadowLayer = {
  color: string;
  x: number;
  y: number;
  blur: number;
  spread: number;
  inset: boolean;
};

function shadowLayers(value: string): ShadowLayer[] {
  if (value === "none" || value === "") return [];
  // Split on commas outside parentheses.
  const layers: string[] = [];
  let depth = 0;
  let start = 0;
  for (let index = 0; index < value.length; index += 1) {
    const char = value[index];
    if (char === "(") depth += 1;
    else if (char === ")") depth -= 1;
    else if (char === "," && depth === 0) {
      layers.push(value.slice(start, index));
      start = index + 1;
    }
  }
  layers.push(value.slice(start));
  return layers.map((layer) => {
    const color =
      layer.match(/[a-z]+\((?:[^()]|\([^()]*\))*\)|#[0-9a-f]+/i)?.[0] ?? "";
    const rest = layer.replace(color, " ");
    const lengths = [...rest.matchAll(/-?[\d.]+px|\b0\b/g)].map((match) =>
      parseFloat(match[0]),
    );
    const [x = 0, y = 0, blur = 0, spread = 0] = lengths;
    return { color, x, y, blur, spread, inset: /\binset\b/.test(rest) };
  });
}

/** The layers that paint something. */
function visibleLayers(element: Element) {
  return shadowLayers(getComputedStyle(element).boxShadow).filter(
    (layer) =>
      tokenOf(layer.color, element) !== "transparent" &&
      (layer.blur > 0 || layer.spread > 0 || layer.x !== 0 || layer.y !== 0),
  );
}

/**
 * A floating surface: lifted on the raised ground, edged by one hairline and
 * shadowed by one soft, offset shadow in theme ink.
 */
function expectFloating(surface: HTMLElement) {
  expect(surface.closest("[data-surface]")).toBe(surface);
  expect(surface).toHaveAttribute("data-surface", "raised");
  const page = document.documentElement;
  expect(
    sameColor(getComputedStyle(surface).backgroundColor, token(page, "raised")),
  ).toBe(true);
  expect(literalColorsIn(surface)).toEqual([]);

  const layers = visibleLayers(surface);
  const soft = layers.filter((layer) => layer.blur > 0);
  const hairlines = layers.filter(
    (layer) => layer.blur === 0 && layer.spread === 1,
  );
  expect(soft.length).toBeGreaterThan(0);
  for (const layer of soft) {
    expect(layer.y).toBeGreaterThan(0);
    expect(layer.blur).toBeGreaterThanOrEqual(16);
    expect(tokenOf(layer.color, page)).toBeDefined();
  }
  expect(hairlines.map((layer) => tokenOf(layer.color, surface))).toEqual([
    "edge-strong",
  ]);
  return getComputedStyle(surface).boxShadow;
}

function radiusOf(element: Element) {
  return getComputedStyle(element).borderTopLeftRadius;
}

async function openByKeyboard(user: UserEvent, trigger: HTMLElement) {
  trigger.focus();
  expect(trigger).toHaveFocus();
  await user.keyboard("{Enter}");
}

/** Focus stays inside `surface` however far the candidate tabs. */
async function expectFocusTrapped(user: UserEvent, surface: HTMLElement) {
  for (let step = 0; step < 4; step += 1) {
    await user.tab();
    expect(surface).toContainElement(document.activeElement as HTMLElement);
  }
  for (let step = 0; step < 4; step += 1) {
    await user.tab({ shift: true });
    expect(surface).toContainElement(document.activeElement as HTMLElement);
  }
}

/** Presses Escape and lets the surface finish animating out. */
async function closeByEscape(user: UserEvent, surface: HTMLElement) {
  await user.keyboard("{Escape}");
  endAnimation(surface);
}

type Overlay = {
  name: string;
  role: "menu" | "dialog" | "alertdialog";
  trigger: string;
  ui: ReactNode;
  corners: string;
};

const overlays: Overlay[] = [
  {
    name: "a menu",
    role: "menu",
    trigger: "Course menu",
    corners: "8px",
    ui: (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon" aria-label="Course menu">
            …
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem>Rename course</DropdownMenuItem>
          <DropdownMenuItem>Export PDF</DropdownMenuItem>
          <DropdownMenuItem>Delete course</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    ),
  },
  {
    name: "a popover",
    role: "dialog",
    trigger: "asthenosphere",
    corners: "12px",
    ui: (
      <Popover>
        <PopoverTrigger asChild>
          <Button variant="ghost">asthenosphere</Button>
        </PopoverTrigger>
        <PopoverContent aria-label="Definition">
          <p>The weak, ductile layer of the upper mantle.</p>
          <a href="#wikipedia">Wikipedia</a>
          <Button>Ask</Button>
        </PopoverContent>
      </Popover>
    ),
  },
  {
    name: "a dialog",
    role: "dialog",
    trigger: "Rename course",
    corners: "12px",
    ui: (
      <Dialog>
        <DialogTrigger asChild>
          <Button>Rename course</Button>
        </DialogTrigger>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Rename course</DialogTitle>
            <DialogDescription>
              The library shows the new name.
            </DialogDescription>
          </DialogHeader>
          <Input aria-label="Name" defaultValue="Plate tectonics" />
          <DialogFooter>
            <Button variant="primary">Save</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    ),
  },
  {
    name: "an alert dialog",
    role: "alertdialog",
    trigger: "Delete course",
    corners: "12px",
    ui: (
      <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button>Delete course</Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Plate tectonics?</AlertDialogTitle>
            <AlertDialogDescription>
              Its sections, side chats and glossary go with it.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Keep course</AlertDialogCancel>
            <AlertDialogAction>Delete course</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    ),
  },
  {
    name: "a sheet",
    role: "dialog",
    trigger: "Side chats",
    corners: "0px",
    ui: (
      <Sheet>
        <SheetTrigger asChild>
          <Button>Side chats</Button>
        </SheetTrigger>
        <SheetContent>
          <SheetHeader>
            <SheetTitle>Side chats</SheetTitle>
            <SheetDescription>Three chats in this course.</SheetDescription>
          </SheetHeader>
          <Button>New side chat</Button>
        </SheetContent>
      </Sheet>
    ),
  },
];

describe.each(schemes)("in the %s theme", (scheme) => {
  describe.each(overlays)("$name", ({ role, trigger, ui, corners }) => {
    it("opens by keyboard, keeps focus inside, closes on Escape and returns focus", async () => {
      const user = userEvent.setup();
      render(ui);
      await applyStyles(scheme);
      const opener = screen.getByRole("button", { name: trigger });

      await openByKeyboard(user, opener);
      const surface = await screen.findByRole(role);
      await waitFor(() =>
        expect(surface).toContainElement(document.activeElement as HTMLElement),
      );
      await expectFocusTrapped(user, surface);

      await closeByEscape(user, surface);
      await waitFor(() => expect(screen.queryByRole(role)).toBeNull());
      expect(opener).toHaveFocus();
    });

    it("floats on the raised surface with the one soft shadow", async () => {
      const user = userEvent.setup();
      render(ui);
      await openByKeyboard(user, screen.getByRole("button", { name: trigger }));
      const surface = await screen.findByRole(role);
      await applyStyles(scheme);

      expectFloating(surface);
      expect(radiusOf(surface)).toBe(corners);
      for (const backdrop of document.querySelectorAll("[data-slot$=overlay]"))
        expect(literalColorsIn(backdrop)).toEqual([]);
    });
  });

  it("a hover card opens on focus, floats raised and closes on Escape", async () => {
    const user = userEvent.setup();
    render(
      <HoverCard openDelay={0} closeDelay={0}>
        <HoverCardTrigger href="#s1">1</HoverCardTrigger>
        <HoverCardContent>Understanding plate motions</HoverCardContent>
      </HoverCard>,
    );
    await user.tab();
    const card = (
      await screen.findByText("Understanding plate motions")
    ).closest("[data-slot=hover-card-content]") as HTMLElement;
    await applyStyles(scheme);

    expectFloating(card);
    expect(radiusOf(card)).toBe("12px");
    await closeByEscape(user, card);
    await waitFor(() =>
      expect(screen.queryByText("Understanding plate motions")).toBeNull(),
    );
  });

  it("a tooltip opens on focus, floats raised and closes on Escape", async () => {
    const user = userEvent.setup();
    render(
      <TooltipProvider delayDuration={0}>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" aria-label="New side chat">
              +
            </Button>
          </TooltipTrigger>
          <TooltipContent>New side chat</TooltipContent>
        </Tooltip>
      </TooltipProvider>,
    );
    await user.tab();
    const tooltip = await screen.findByRole("tooltip");
    const surface = tooltip.closest(
      "[data-slot=tooltip-content]",
    ) as HTMLElement;
    await applyStyles(scheme);

    expectFloating(surface);
    expect(radiusOf(surface)).toBe("6px");
    await closeByEscape(user, surface);
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
  });

  it("every floating surface takes the same shadow", async () => {
    const user = userEvent.setup();
    const shadows = new Set<string>();
    for (const { role, trigger, ui } of overlays) {
      const { unmount } = render(ui);
      await openByKeyboard(user, screen.getByRole("button", { name: trigger }));
      const surface = await screen.findByRole(role);
      await applyStyles(scheme);
      shadows.add(getComputedStyle(surface).boxShadow);
      unmount();
    }
    expect(shadows.size).toBe(1);
  });

  it("a menu shows keyboard focus on every kind of item in the accent", async () => {
    const user = userEvent.setup();
    render(
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost">Claude Sonnet</Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem>Rename course</DropdownMenuItem>
          <DropdownMenuCheckboxItem checked>
            Show figures
          </DropdownMenuCheckboxItem>
          <DropdownMenuRadioGroup value="sonnet">
            <DropdownMenuRadioItem value="sonnet">
              Claude Sonnet
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
          <DropdownMenuSub>
            <DropdownMenuSubTrigger>Export</DropdownMenuSubTrigger>
            <DropdownMenuSubContent>
              <DropdownMenuItem>PDF</DropdownMenuItem>
            </DropdownMenuSubContent>
          </DropdownMenuSub>
        </DropdownMenuContent>
      </DropdownMenu>,
    );
    await openByKeyboard(
      user,
      screen.getByRole("button", { name: "Claude Sonnet" }),
    );
    await screen.findByRole("menu");
    await applyStyles(scheme);

    const items = [
      screen.getByRole("menuitem", { name: "Rename course" }),
      screen.getByRole("menuitemcheckbox", { name: "Show figures" }),
      screen.getByRole("menuitemradio", { name: "Claude Sonnet" }),
      screen.getByRole("menuitem", { name: "Export" }),
    ];
    await waitFor(() => expect(items[0]).toHaveFocus());
    for (const [index, item] of items.entries()) {
      if (index > 0) await user.keyboard("{ArrowDown}");
      expect(item).toHaveFocus();
      const style = getComputedStyle(item);
      expect(style.outlineStyle).toBe("solid");
      expect(style.outlineWidth).toBe("2px");
      expect(sameColor(style.outlineColor, token(item, "focus"))).toBe(true);
    }
  });
});
