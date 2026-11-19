// AI Elements: Inline Citation (https://ui.shadcn.com/docs/ai-elements/radix/inline-citation).
// A source in the sentence that opens on hover.
import { Button } from "@/components/ui/button";
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@/components/ui/hover-card";

export function InlineCitation({
  url,
  title,
}: {
  url: string;
  title?: string;
}) {
  const hostname = new URL(url).hostname;
  return (
    <HoverCard openDelay={10} closeDelay={100}>
      <HoverCardTrigger asChild>
        <Button variant="secondary" size="xs">
          {hostname}
        </Button>
      </HoverCardTrigger>
      <HoverCardContent className="flex w-64 flex-col gap-1">
        <div className="font-medium">{title ?? hostname}</div>
        <a href={url} target="_blank" rel="noreferrer">
          {hostname}
          <span className="sr-only"> (opens in a new tab)</span>
        </a>
      </HoverCardContent>
    </HoverCard>
  );
}
