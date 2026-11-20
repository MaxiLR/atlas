// AI Elements: Reasoning (https://ui.shadcn.com/docs/ai-elements/radix/reasoning).
// The agent's thinking as it streams.
import { CircleCheckIcon } from "lucide-react";

import {
  Trace,
  TraceCollapsible,
  TraceCollapsibleContent,
  TraceCollapsibleTrigger,
  TraceDescription,
  TraceIcon,
  TraceLine,
  TraceTitle,
} from "@/components/ui/trace";

export function Reasoning({
  text,
  title = "Thought",
  seconds,
}: {
  text: string;
  title?: string;
  seconds?: number;
}) {
  return (
    <Trace>
      <TraceCollapsible>
        <TraceCollapsibleTrigger>
          <TraceLine>
            <TraceIcon>
              <CircleCheckIcon />
            </TraceIcon>
            <TraceTitle>{title}</TraceTitle>
            {seconds !== undefined && (
              <TraceDescription>{seconds}s</TraceDescription>
            )}
          </TraceLine>
        </TraceCollapsibleTrigger>
        <TraceCollapsibleContent className="whitespace-pre-wrap">
          {text}
        </TraceCollapsibleContent>
      </TraceCollapsible>
    </Trace>
  );
}
