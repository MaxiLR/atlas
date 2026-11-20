// AI Elements: Tool (https://ui.shadcn.com/docs/ai-elements/radix/tool).
// A tool call the agent makes: a muted row naming what it did and its
// result, which opens on the output when there is one.
import type { ReactNode } from "react";

import { CodeBlock, CodeBlockContent } from "@/components/ui/code-block";
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

export function Tool({
  icon,
  title,
  description,
  output,
}: {
  icon: ReactNode;
  title: ReactNode;
  /** The result in a few words, such as "2 sources". */
  description?: ReactNode;
  output?: string;
}) {
  const line = (
    <TraceLine className="w-full">
      <TraceIcon>{icon}</TraceIcon>
      <TraceTitle>{title}</TraceTitle>
      {description !== undefined && (
        <TraceDescription className="ml-auto pl-2">
          {description}
        </TraceDescription>
      )}
    </TraceLine>
  );

  if (output === undefined) return <Trace>{line}</Trace>;

  return (
    <Trace>
      <TraceCollapsible>
        <TraceCollapsibleTrigger>{line}</TraceCollapsibleTrigger>
        <TraceCollapsibleContent>
          <CodeBlock size="sm">
            <CodeBlockContent wrap className="max-h-64">
              {output}
            </CodeBlockContent>
          </CodeBlock>
        </TraceCollapsibleContent>
      </TraceCollapsible>
    </Trace>
  );
}
