import { ChevronLeftIcon, PenLineIcon } from "lucide-react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Chat, type ChatTurn } from "@/components/ai-elements/chat";
import { PersonMessage } from "@/components/ai-elements/message";
import { PromptInput } from "@/components/ai-elements/prompt-input";
import { Suggestions } from "@/components/ai-elements/suggestion";
import { Bubble, BubbleContent } from "@/components/ui/bubble";
import { Button } from "@/components/ui/button";
import { Trace, TraceIcon, TraceLine, TraceTitle } from "@/components/ui/trace";
import { ModelPicker } from "@/model-picker/model-picker";
import { replyAwaited } from "@/store/side-chats";
import { outcomeError } from "@/store/stream";

import { ReplyFailure, StoredReply, StreamingReply } from "./reply";

/**
 * One side chat: its passage quoted with its section, the thread, the reply
 * being written with Stop, what went wrong with a way on, up to three
 * follow-up prompts, and the prompt field with the model picker in its row.
 * The field takes focus when the chat opens.
 */
export function SideChatThread({
  chatId,
  onBack,
}: {
  chatId: string;
  onBack: () => void;
}) {
  const { send } = useAtlasStore();
  const open = useAtlas((state) => state.sideChats.chats[chatId]);
  if (open === undefined) return null;
  const { chat, thread, reply, followUps } = open;
  const error = open.error ?? outcomeError(reply?.outcome ?? null);
  const last = thread.at(-1);
  const lastQuestion =
    last?.type === "Sent"
      ? last.text
      : last?.message.role === "Candidate"
        ? last.message.text
        : null;
  const pending = last?.type === "Sent" && reply === null && error === null;
  const streaming = replyAwaited(open);

  const ask = (text: string) => {
    send({ type: "SendSideChatMessage", chatId, text }).catch(failureInStore);
  };

  const turns: ChatTurn[] = [];
  if (chat.anchor !== null) {
    turns.push({
      id: "passage",
      fromPerson: false,
      content: (
        <Bubble variant="tinted" className="max-w-full">
          <BubbleContent asChild>
            <blockquote>
              <p>{chat.anchor.quote}</p>
              {chat.sectionTitle !== null && (
                <footer className="mt-1.5 text-12 text-muted-foreground">
                  <cite className="not-italic">{chat.sectionTitle}</cite>
                </footer>
              )}
            </blockquote>
          </BubbleContent>
        </Bubble>
      ),
    });
  }
  thread.forEach((item, at) => {
    if (item.type === "Sent" || item.message.role === "Candidate") {
      const text = item.type === "Sent" ? item.text : item.message.text;
      turns.push({
        id: item.type === "Sent" ? `sent-${at}` : item.message.id,
        fromPerson: true,
        content: (
          <PersonMessage>
            <span className="whitespace-pre-wrap">{text}</span>
          </PersonMessage>
        ),
      });
    } else {
      turns.push({
        id: item.message.id,
        fromPerson: false,
        content: <StoredReply message={item.message} />,
      });
    }
  });
  if (reply !== null) {
    turns.push({
      id: `reply-${reply.generationId}`,
      fromPerson: false,
      content: <StreamingReply stream={reply} searched={open.searched} />,
    });
  } else if (pending) {
    turns.push({
      id: "pending",
      fromPerson: false,
      content: (
        <Trace role="status">
          <TraceLine status="running">
            <TraceIcon>
              <PenLineIcon />
            </TraceIcon>
            <TraceTitle>Writing a reply…</TraceTitle>
          </TraceLine>
        </Trace>
      ),
    });
  }
  if (error !== null) {
    turns.push({
      id: "failure",
      fromPerson: false,
      content: (
        <ReplyFailure
          error={error}
          onSendAgain={lastQuestion === null ? null : () => ask(lastQuestion)}
        />
      ),
    });
  }
  if (!streaming && error === null && followUps.length > 0) {
    turns.push({
      id: "follow-ups",
      fromPerson: false,
      content: <Suggestions suggestions={followUps.slice(0, 3)} onSend={ask} />,
    });
  }

  const label = thread.length === 0 ? "Ask a question" : "Ask a follow-up";
  return (
    <section aria-label="Side chat" className="flex min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 items-center px-2 pt-2">
        <Button variant="ghost" onClick={onBack}>
          <ChevronLeftIcon aria-hidden />
          All side chats
        </Button>
      </div>
      <div className="min-h-0 flex-1">
        <Chat turns={turns} className="gap-4 px-4 pt-3 pb-5" />
      </div>
      <div className="shrink-0 px-3 pb-3">
        <PromptInput
          key={chatId}
          label={label}
          placeholder={label}
          autoFocus
          onSend={ask}
          onStop={
            streaming
              ? () =>
                  send({ type: "StopSideChatReply", chatId }).catch(
                    failureInStore,
                  )
              : undefined
          }
          addons={<ModelPicker />}
        />
      </div>
    </section>
  );
}
