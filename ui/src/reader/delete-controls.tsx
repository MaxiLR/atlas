import { EllipsisIcon, Trash2Icon } from "lucide-react";
import { useEffect, useEffectEvent, useRef } from "react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

// A glossary entry or a side chat is cheap to make again, so each is
// deleted at once and for good: no dialog, no Undo, no notice.

/**
 * A ghost icon button with the trash icon that deletes what it stands in,
 * named `label` and showing that name as a tooltip on hover and focus.
 */
export function DeleteButton({
  label,
  onDelete,
}: {
  label: string;
  onDelete: () => void;
}) {
  return (
    <TooltipProvider>
      {/* The tooltip only repeats the name: it goes as the pointer leaves. */}
      <Tooltip disableHoverableContent>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            aria-label={label}
            onClick={onDelete}
          >
            <Trash2Icon aria-hidden />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{label}</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}

/**
 * A row's ⋯ menu, as the library's course row has it, with one item that
 * deletes the row. The menu's second click is the only guard.
 */
export function DeleteMenu({
  label,
  item,
  describedBy,
  rowId,
  onDelete,
  className,
}: {
  /** The menu button's name, e.g. "Side chat menu". */
  label: string;
  /** The item's text, e.g. "Delete side chat". */
  item: string;
  /** The id of the text that names the row. */
  describedBy: string;
  /** Marks the button as `data-row-menu`, so focus can find it. */
  rowId: string;
  onDelete: () => void;
  className?: string;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          aria-label={label}
          aria-describedby={describedBy}
          data-row-menu={rowId}
          className={className}
        >
          <EllipsisIcon aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onSelect={onDelete}>
          <Trash2Icon aria-hidden className="text-destructive" />
          {item}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/**
 * The row that takes the place of `id` in `ids` once it is deleted: the
 * next one, else the previous one; null when none is left or `id` is not
 * among them.
 */
export function neighbourOf(ids: readonly string[], id: string): string | null {
  const at = ids.indexOf(id);
  if (at === -1) return null;
  return ids[at + 1] ?? ids[at - 1] ?? null;
}

/**
 * Moves focus once a deleted row has left its list (`ids`, null while the
 * list loads): `focus` receives the row now in its place, else the one
 * before it, else null for the list's empty message. `deleting(id)` names
 * the row on its way out; `kept()` forgets it when the core refuses.
 */
export function useFocusAfterDelete(
  ids: readonly string[] | null,
  focus: (id: string | null) => void,
) {
  const pending = useRef<{ id: string; neighbour: string | null } | null>(null);
  const moveFocus = useEffectEvent(focus);
  useEffect(() => {
    const deleted = pending.current;
    if (deleted === null || ids === null || ids.includes(deleted.id)) return;
    pending.current = null;
    moveFocus(deleted.neighbour);
  }, [ids]);
  return {
    deleting(id: string) {
      pending.current = { id, neighbour: neighbourOf(ids ?? [], id) };
    },
    kept() {
      pending.current = null;
    },
  };
}
