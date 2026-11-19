// AI Elements: Inline Citation (https://ui.shadcn.com/docs/ai-elements/radix/inline-citation).
// A numbered chip in the sentence; hover or focus opens its source.
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@/components/ui/hover-card";

export function InlineCitation({
  number,
  url,
  title,
  site,
}: {
  /** The source's number in the list of sources. */
  number: number;
  url: string;
  title: string;
  /** Who published it; the link's host name when unknown. */
  site?: string;
}) {
  const publisher = site ?? new URL(url).hostname;
  return (
    <HoverCard openDelay={10} closeDelay={100}>
      <HoverCardTrigger asChild>
        <button
          type="button"
          aria-label={`Source ${number}: ${title}`}
          className="ml-0.5 inline-block min-w-4 cursor-pointer rounded-[4px] border border-edge-strong px-1 text-center align-[2px] text-[11px] leading-[14px] tracking-normal text-muted-foreground tabular-nums transition-colors outline-none hover:bg-wash hover:text-heading focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
        >
          {number}
        </button>
      </HoverCardTrigger>
      <HoverCardContent className="flex flex-col gap-1">
        <a
          href={url}
          target="_blank"
          rel="noreferrer"
          className="rounded-sm font-medium text-heading underline decoration-edge-strong underline-offset-3 outline-none hover:decoration-current focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
        >
          {title}
          <span className="sr-only"> (opens in a new tab)</span>
        </a>
        <span className="text-muted-foreground">{publisher}</span>
      </HoverCardContent>
    </HoverCard>
  );
}
