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

/** The reply is set in Typeset at the side panel's 15 px. */
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
        <div className="typeset [--typeset-flow:0.75em] [--typeset-leading:1.6] [--typeset-size:0.9375rem]">
          {children}
        </div>
        {actions !== undefined && (
          <MessageFooter className="px-0">{actions}</MessageFooter>
        )}
      </MessageContent>
    </Message>
  );
}
