import { useAtlas } from "@/app/store";

import { SideChatList } from "./side-chat-list";

/** The Side chats tab: the course's list, or the chat open in it. */
export function SideChats({ courseId }: { courseId: string }) {
  const openChatId = useAtlas((state) => state.navigation.panel.chatId);
  return <SideChatList courseId={courseId} openChatId={openChatId} />;
}
