import type { ChatMessage } from "@/core/generated/ChatMessage";
import type { CoreError } from "@/core/generated/CoreError";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { SideChat } from "@/core/generated/SideChat";
import type { SideChatSummary } from "@/core/generated/SideChatSummary";

import { applyGeneration, type Stream } from "./stream";

/**
 * A line of a thread: a message the core holds, or the candidate's message
 * as sent, which the core returns once the chat is opened again after its
 * reply.
 */
export type ThreadItem =
  { type: "Message"; message: ChatMessage } | { type: "Sent"; text: string };

export interface OpenChat {
  chat: SideChat;
  thread: ThreadItem[];
  /** The reply streaming, or the last one when it stopped or failed. */
  reply: Stream | null;
  /**
   * The reply generation the core last answered for: its events can still
   * arrive after the response, which travels apart from them, and change
   * nothing.
   */
  settled: string | null;
  /** Whether the agent searched the web while writing `reply`. */
  searched: boolean;
  followUps: string[];
  /** Why the last message sent got no reply; null once another is sent. */
  error: CoreError | null;
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

/**
 * Whether the chat's reply is still to come: the core has not begun it, or
 * it is streaming.
 */
export function replyAwaited(chat: OpenChat): boolean {
  if (chat.error !== null) return false;
  return chat.reply === null
    ? chat.thread.at(-1)?.type === "Sent"
    : chat.reply.outcome === null;
}

/**
 * The core's copy of the chat. Opened again while its reply is awaited, the
 * chat keeps its thread and the reply as streamed so far, which the core's
 * copy does not hold.
 */
export function chatOpened(
  slice: SideChatsSlice,
  chat: SideChat,
): SideChatsSlice {
  const known = slice.chats[chat.id];
  return {
    ...slice,
    chats: {
      ...slice.chats,
      [chat.id]:
        known !== undefined && replyAwaited(known)
          ? { ...known, chat }
          : {
              chat,
              thread: chat.messages.map((message) => ({
                type: "Message",
                message,
              })),
              reply: null,
              settled: known?.settled ?? null,
              searched: false,
              followUps: chat.followUps,
              error: null,
            },
    },
  };
}

/**
 * The candidate sent a message. Sent again after it failed, it stays the
 * one line it was.
 */
export function messageSent(
  slice: SideChatsSlice,
  chatId: string,
  text: string,
): SideChatsSlice {
  return update(slice, chatId, (chat) => {
    const last = chat.thread.at(-1);
    const again =
      chat.error !== null &&
      (last?.type === "Sent" ? last.text : last?.message.text) === text;
    return {
      ...chat,
      thread: again ? chat.thread : [...chat.thread, { type: "Sent", text }],
      reply: null,
      searched: false,
      followUps: [],
      error: null,
    };
  });
}

/** The core refused the message, or failed while writing its reply. */
export function sendFailed(
  slice: SideChatsSlice,
  chatId: string,
  error: CoreError,
): SideChatsSlice {
  return update(slice, chatId, (chat) => ({ ...chat, error }));
}

/** One step of a generation writing a reply. */
export function replyGenerated(
  slice: SideChatsSlice,
  chatId: string,
  generationId: string,
  event: GenerationEvent,
): SideChatsSlice {
  return update(slice, chatId, (chat) => {
    if (generationId === chat.settled) return chat;
    const reply = applyGeneration(chat.reply, generationId, event);
    return {
      ...chat,
      reply,
      searched:
        (chat.searched && chat.reply?.generationId === generationId) ||
        reply.activity?.type === "Searching",
    };
  });
}

/**
 * The finished reply joins the thread in place of its stream; the stream's
 * late events, if any, are ignored.
 */
export function replied(
  slice: SideChatsSlice,
  chatId: string,
  message: ChatMessage,
): SideChatsSlice {
  return update(slice, chatId, (chat) => ({
    ...chat,
    thread: [...chat.thread, { type: "Message", message }],
    reply: null,
    settled: chat.reply?.generationId ?? chat.settled,
  }));
}

export function followUpsOffered(
  slice: SideChatsSlice,
  chatId: string,
  prompts: string[],
): SideChatsSlice {
  return update(slice, chatId, (chat) => ({ ...chat, followUps: prompts }));
}
