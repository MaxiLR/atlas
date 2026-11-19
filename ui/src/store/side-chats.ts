import type { ChatMessage } from "@/core/generated/ChatMessage";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { SideChat } from "@/core/generated/SideChat";
import type { SideChatSummary } from "@/core/generated/SideChatSummary";

import { applyGeneration, type Stream } from "./stream";

/**
 * A line of a thread: a message the core holds, or the candidate's message
 * as sent, which the core returns only when the chat is opened again.
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
  /**
   * Each course's side chats heard of, newest first: all of them once the
   * core has listed them (`listed`), otherwise those its events named.
   */
  byCourse: Record<string, SideChatSummary[]>;
  /** The courses whose side chats the core has listed. */
  listed: Record<string, true>;
  /** The chats opened, by id. */
  chats: Record<string, OpenChat>;
}

export const initialSideChats: SideChatsSlice = {
  byCourse: {},
  listed: {},
  chats: {},
};

export function chatsListed(
  slice: SideChatsSlice,
  courseId: string,
  chats: SideChatSummary[],
): SideChatsSlice {
  return {
    ...slice,
    byCourse: { ...slice.byCourse, [courseId]: chats },
    listed: { ...slice.listed, [courseId]: true },
  };
}

/** A chat was started (it goes first) or received a message. */
export function chatChanged(
  slice: SideChatsSlice,
  summary: SideChatSummary,
): SideChatsSlice {
  const chats = slice.byCourse[summary.courseId] ?? [];
  const at = chats.findIndex((chat) => chat.id === summary.id);
  return {
    ...slice,
    byCourse: {
      ...slice.byCourse,
      [summary.courseId]:
        at === -1 ? [summary, ...chats] : chats.with(at, summary),
    },
  };
}

function update(
  slice: SideChatsSlice,
  chatId: string,
  change: (chat: OpenChat) => OpenChat,
): SideChatsSlice {
  const chat = slice.chats[chatId];
  return chat === undefined
    ? slice
    : { ...slice, chats: { ...slice.chats, [chatId]: change(chat) } };
}

export function chatOpened(
  slice: SideChatsSlice,
  chat: SideChat,
): SideChatsSlice {
  return {
    ...slice,
    chats: {
      ...slice.chats,
      [chat.id]: {
        chat,
        thread: chat.messages.map((message) => ({ type: "Message", message })),
        reply: null,
        followUps: chat.followUps,
      },
    },
  };
}

export function messageSent(
  slice: SideChatsSlice,
  chatId: string,
  text: string,
): SideChatsSlice {
  return update(slice, chatId, (chat) => ({
    ...chat,
    thread: [...chat.thread, { type: "Sent", text }],
    followUps: [],
  }));
}

/** One step of a generation writing a reply. */
export function replyGenerated(
  slice: SideChatsSlice,
  chatId: string,
  generationId: string,
  event: GenerationEvent,
): SideChatsSlice {
  return update(slice, chatId, (chat) => ({
    ...chat,
    reply: applyGeneration(chat.reply, generationId, event),
  }));
}

/** The finished reply joins the thread in place of its stream. */
export function replied(
  slice: SideChatsSlice,
  chatId: string,
  message: ChatMessage,
): SideChatsSlice {
  return update(slice, chatId, (chat) => ({
    ...chat,
    thread: [...chat.thread, { type: "Message", message }],
    reply: null,
  }));
}

export function followUpsOffered(
  slice: SideChatsSlice,
  chatId: string,
  prompts: string[],
): SideChatsSlice {
  return update(slice, chatId, (chat) => ({ ...chat, followUps: prompts }));
}
