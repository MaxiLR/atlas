// AI Elements: Task (https://ui.shadcn.com/docs/ai-elements/radix/task).
// The steps the agent takes, as it takes them.
import type { ReactNode } from "react";

import {
  Trace,
  TraceCollapsible,
  TraceCollapsibleContent,
  TraceCollapsibleTrigger,
  TraceIcon,
  TraceLine,
  TraceTitle,
} from "@/components/ui/trace";

export interface TaskStep {
  id: string;
  icon: ReactNode;
  title: ReactNode;
}

export function Task({
  title,
  steps,
}: {
  title: ReactNode;
  steps: TaskStep[];
}) {
  return (
    <Trace>
      <TraceCollapsible defaultOpen>
        <TraceCollapsibleTrigger>
          <TraceLine>
            <TraceTitle>{title}</TraceTitle>
          </TraceLine>
        </TraceCollapsibleTrigger>
        <TraceCollapsibleContent>
          <Trace>
            {steps.map((step) => (
              <TraceLine key={step.id}>
                <TraceIcon>{step.icon}</TraceIcon>
                <TraceTitle>{step.title}</TraceTitle>
              </TraceLine>
            ))}
          </Trace>
        </TraceCollapsibleContent>
      </TraceCollapsible>
    </Trace>
  );
}
