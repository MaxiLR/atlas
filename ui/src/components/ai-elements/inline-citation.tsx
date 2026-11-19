// AI Elements: Inline Citation (https://ui.shadcn.com/docs/ai-elements/radix/inline-citation).
// A numbered chip in the sentence; hover or focus opens its source.
import { ArrowUpRightIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@/components/ui/hover-card";

import { siteOf } from "./sources";

/**
 * Activating the chip, or the card's control, asks `onOpen` to open the
 * source in the system browser; the window itself never follows the link.
 * Escape closes the card and leaves focus on the chip.
 */
export function InlineCitation({
  number,
  url,
  title,
  site,
  onOpen,
  describedBy,
}: {
  /** The source's number in the list of sources. */
  number: number;
  url: string;
  title: string;
  /** Who published it; the link's host name when unknown. */
  site?: string;
  onOpen?: () => void;
  /** The id of the source's entry in the list of sources. */
  describedBy?: string;
}) {
  const publisher = site ?? siteOf(url);
  return (
    <HoverCard openDelay={10} closeDelay={100}>
      <HoverCardTrigger asChild>
        <button
          type="button"
          aria-label={`Source ${number}: ${title}`}
          aria-describedby={describedBy}
          onClick={onOpen}
          className="ml-0.5 inline-block min-w-4 cursor-pointer rounded-[4px] border border-edge-strong px-1 text-center align-[2px] text-[11px] leading-[14px] tracking-normal text-muted-foreground tabular-nums transition-colors outline-none hover:bg-wash hover:text-heading focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
        >
          {number}
        </button>
      </HoverCardTrigger>
      <HoverCardContent
        role="dialog"
        aria-label={`Source ${number}`}
        collisionPadding={8}
        className="w-80 overflow-hidden p-0"
      >
        <p className="px-4 pt-3.5 pb-3 text-15 font-medium text-pretty text-heading">
          {title}
        </p>
        <div className="flex items-center justify-between gap-3 border-t border-edge bg-wash py-2 pr-2 pl-4 text-muted-foreground">
          <span className="min-w-0 truncate">{publisher}</span>
          <Button asChild>
            <a
              href={url}
              onClick={(event) => {
                event.preventDefault();
                onOpen?.();
              }}
              onAuxClick={(event) => event.preventDefault()}
            >
              Open in browser
              <ArrowUpRightIcon aria-hidden="true" />
            </a>
          </Button>
        </div>
      </HoverCardContent>
    </HoverCard>
  );
}
