import { useState } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { neighbourOf } from "@/reader/delete-controls";
import { selectSideChats } from "@/store";

import { SideChatList, type ListFocus } from "./side-chat-list";
import { SideChatThread } from "./side-chat-thread";

/**
 * The Side chats tab: the open chat of this course, or the course's list.
 * Leaving a chat for the list puts focus back on its entry; deleting it
 * puts focus where its entry was: the row now in its place, else the one
 * before, else the empty message.
 */
export function SideChats({ courseId }: { courseId: string }) {
  const { send, navigate, state } = useAtlasStore();
  const openChatId = useAtlas((current) => {
    const { chatId } = current.navigation.panel;
    return chatId !== null &&
      current.sideChats.chats[chatId]?.chat.courseId === courseId
      ? chatId
      : null;
  });
  const [focus, setFocus] = useState<ListFocus | null>(null);

  if (openChatId !== null) {
    return (
      <SideChatThread
        chatId={openChatId}
        onBack={() => {
          setFocus({ type: "Row", chatId: openChatId });
          navigate({ type: "ShowSideChatList" });
        }}
        onDelete={() => {
          const ids = (selectSideChats(state.getState(), courseId) ?? []).map(
            ({ id }) => id,
          );
          const neighbour = neighbourOf(ids, openChatId);
          setFocus(
            neighbour === null
              ? { type: "Empty" }
              : { type: "Row", chatId: neighbour },
          );
          // Done shows the list; refused, the chat stays open.
          send({ type: "DeleteSideChat", chatId: openChatId }).catch(
            failureInStore,
          );
        }}
      />
    );
  }
  return <SideChatList courseId={courseId} focus={focus} />;
}
