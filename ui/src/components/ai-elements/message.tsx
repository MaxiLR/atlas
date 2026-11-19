// AI Elements: Message (https://ui.shadcn.com/docs/ai-elements/radix/message).
// One turn: the person's message in a bubble, the agent's reply with actions.
import type { ReactNode } from "react";

import { Bubble, BubbleContent } from "@/components/ui/bubble";
import {
  Message,
  MessageContent,
  MessageFooter,
} from "@/components/ui/message";

export function PersonMessage({ children }: { children: ReactNode }) {
  return (
    <Message align="end">
      <MessageContent>
        <Bubble align="end" variant="muted">
          <BubbleContent>{children}</BubbleContent>
        </Bubble>
      </MessageContent>
    </Message>
  );
}

export function AgentMessage({
  children,
  actions,
}: {
  children: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <Message>
      <MessageContent className="gap-2">
        {children}
        {actions !== undefined && <MessageFooter>{actions}</MessageFooter>}
      </MessageContent>
    </Message>
  );
}
