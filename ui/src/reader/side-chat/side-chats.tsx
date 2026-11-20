import { useState } from "react";

import { useAtlas, useAtlasStore } from "@/app/store";

import { SideChatList } from "./side-chat-list";
import { SideChatThread } from "./side-chat-thread";

/**
 * The Side chats tab: the open chat of this course, or the course's list.
 * Leaving a chat for the list puts focus back on its entry.
 */
export function SideChats({ courseId }: { courseId: string }) {
  const { navigate } = useAtlasStore();
  const openChatId = useAtlas((state) => {
    const { chatId } = state.navigation.panel;
    return chatId !== null &&
      state.sideChats.chats[chatId]?.chat.courseId === courseId
      ? chatId
      : null;
  });
  const [left, setLeft] = useState<string | null>(null);

  if (openChatId !== null) {
    return (
      <SideChatThread
        chatId={openChatId}
        onBack={() => {
          setLeft(openChatId);
          navigate({ type: "ShowSideChatList" });
        }}
      />
    );
  }
  return <SideChatList courseId={courseId} focusChatId={left} />;
}
