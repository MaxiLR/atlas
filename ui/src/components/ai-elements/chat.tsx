// AI Elements: Chat (https://ui.shadcn.com/docs/ai-elements/radix/chat).
// The thread; Message Scroller keeps the reader's place while a reply streams.
import type { ReactNode } from "react";

import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from "@/components/ui/message-scroller";

export interface ChatTurn {
  id: string;
  /** The person's turns anchor the scroll position. */
  fromPerson: boolean;
  content: ReactNode;
}

export function Chat({
  turns,
  className,
}: {
  turns: ChatTurn[];
  /** Spacing around and between the turns. */
  className?: string;
}) {
  return (
    <MessageScrollerProvider autoScroll>
      <MessageScroller>
        <MessageScrollerViewport>
          <MessageScrollerContent className={className}>
            {turns.map((turn) => (
              <MessageScrollerItem
                key={turn.id}
                messageId={turn.id}
                scrollAnchor={turn.fromPerson}
              >
                {turn.content}
              </MessageScrollerItem>
            ))}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
      </MessageScroller>
    </MessageScrollerProvider>
  );
}
