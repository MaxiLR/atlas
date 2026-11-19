import type { ChatMessage } from "@/core/generated/ChatMessage";
import type { SideChat } from "@/core/generated/SideChat";
import type { SideChatSummary } from "@/core/generated/SideChatSummary";

import type { Stream } from "./stream";

/**
 * A line of a thread: a message the core holds, or the candidate's message
 * as sent, which the core confirms only when the chat is opened again.
 */
export type ThreadItem =
  { type: "Message"; message: ChatMessage } | { type: "Sent"; text: string };

export interface OpenChat {
  chat: SideChat;
  thread: ThreadItem[];
  /** The reply streaming, or the last one when it stopped or failed. */
  reply: Stream | null;
  followUps: string[];
}

export interface SideChatsSlice {
  /** Each course's side chats, newest first. */
  byCourse: Record<string, SideChatSummary[]>;
  /** The chats opened, by id. */
  chats: Record<string, OpenChat>;
}

export const initialSideChats: SideChatsSlice = { byCourse: {}, chats: {} };
