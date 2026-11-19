// AI Elements: Tool (https://ui.shadcn.com/docs/ai-elements/radix/tool).
// A tool call the agent makes, from input to output.
import type { ReactNode } from "react";

import { CodeBlock, CodeBlockContent } from "@/components/ui/code-block";
import {
  Trace,
  TraceCollapsible,
  TraceCollapsibleContent,
  TraceCollapsibleTrigger,
  TraceIcon,
  TraceLine,
  TraceTitle,
} from "@/components/ui/trace";

export function Tool({
  icon,
  title,
  output,
}: {
  icon: ReactNode;
  title: ReactNode;
  output?: string;
}) {
  return (
    <Trace>
      <TraceCollapsible>
        <TraceCollapsibleTrigger>
          <TraceLine>
            <TraceIcon>{icon}</TraceIcon>
            <TraceTitle>{title}</TraceTitle>
          </TraceLine>
        </TraceCollapsibleTrigger>
        {output !== undefined && (
          <TraceCollapsibleContent>
            <CodeBlock size="sm">
              <CodeBlockContent wrap className="max-h-64">
                {output}
              </CodeBlockContent>
            </CodeBlock>
          </TraceCollapsibleContent>
        )}
      </TraceCollapsible>
    </Trace>
  );
}
