import { useEffect, useEffectEvent, useId, useMemo, useRef } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import type { SideChatSummary } from "@/core/generated/SideChatSummary";
import { ModelPicker } from "@/model-picker/model-picker";
import { DeleteMenu, useFocusAfterDelete } from "@/reader/delete-controls";
import { selectSideChats } from "@/store";

/**
 * What takes focus when the list shows: a chat's row, or the empty message
 * once the last chat is deleted.
 */
export type ListFocus = { type: "Row"; chatId: string } | { type: "Empty" };

/**
 * One chat, by its passage (quoted) or its first question and the section
 * it was asked in; choosing it opens it. Its menu at the top right deletes
 * it.
 */
function SideChatRow({
  chat,
  onDelete,
}: {
  chat: SideChatSummary;
  onDelete: (chat: SideChatSummary) => void;
}) {
  const { send } = useAtlasStore();
  const openingId = useId();
  return (
    <li className="relative grid grid-cols-[minmax(0,1fr)_28px] items-start gap-1 rounded-md py-2 pr-1 pl-2 transition-colors hover:bg-wash has-[[aria-expanded=true]]:bg-wash motion-reduce:transition-none">
      <button
        type="button"
        data-chat-open={chat.id}
        onClick={() =>
          send({ type: "OpenSideChat", chatId: chat.id }).catch(failureInStore)
        }
        className="flex min-w-0 cursor-pointer flex-col gap-0.5 text-left outline-none after:absolute after:inset-0 after:rounded-md focus-visible:after:outline-2 focus-visible:after:outline-offset-2 focus-visible:after:outline-focus focus-visible:after:outline-solid"
      >
        <span id={openingId} className="line-clamp-2 text-13 text-body">
          {chat.opening === ""
            ? "No question yet"
            : chat.anchor === null
              ? chat.opening
              : `“${chat.opening}”`}
        </span>
        {chat.sectionTitle !== null && (
          <span className="truncate text-12 text-muted-foreground">
            {chat.sectionTitle}
          </span>
        )}
      </button>
      <DeleteMenu
        label="Side chat menu"
        item="Delete side chat"
        describedBy={openingId}
        rowId={chat.id}
        onDelete={() => onDelete(chat)}
        className="relative z-10"
      />
    </li>
  );
}

/**
 * The course's side chats, newest first; the model in use at the foot.
 * `focus` names what takes focus once the chats show: the row of the chat
 * just left, or where a chat deleted from its thread was. A chat deleted
 * from its row menu leaves at once; focus moves to the next row, else the
 * previous one, else the empty message.
 */
export function SideChatList({
  courseId,
  focus,
}: {
  courseId: string;
  focus: ListFocus | null;
}) {
  const { send } = useAtlasStore();
  const chats = useAtlas((state) => selectSideChats(state, courseId));
  const ids = useMemo(() => chats?.map((chat) => chat.id) ?? null, [chats]);
  const list = useRef<HTMLUListElement>(null);
  const empty = useRef<HTMLParagraphElement>(null);

  const focusOn = (chatId: string | null) => {
    if (chatId === null) {
      empty.current?.focus();
    } else {
      list.current
        ?.querySelector<HTMLElement>(`[data-chat-open="${chatId}"]`)
        ?.focus();
    }
  };
  const afterDelete = useFocusAfterDelete(ids, focusOn);

  useEffect(() => {
    send({ type: "ListSideChats", courseId }).catch(failureInStore);
  }, [send, courseId]);

  const listed = chats !== null;
  const focusFirst = useEffectEvent(() => {
    if (focus !== null) focusOn(focus.type === "Row" ? focus.chatId : null);
  });
  useEffect(() => {
    if (listed) focusFirst();
  }, [listed]);

  return (
    <>
      <div className="relative min-h-0 flex-1 overflow-y-auto px-2 py-3">
        {chats === null ? (
          <p role="status" className="px-2 text-13 text-muted-foreground">
            Loading side chats…
          </p>
        ) : chats.length === 0 ? (
          <p
            ref={empty}
            tabIndex={-1}
            className="rounded-xs px-2 text-13 text-muted-foreground outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
          >
            No side chats yet: ask about a selected passage, or start one with
            New side chat.
          </p>
        ) : (
          <ul
            ref={list}
            aria-label="Side chats"
            className="flex flex-col gap-0.5"
          >
            {chats.map((chat) => (
              <SideChatRow
                key={chat.id}
                chat={chat}
                onDelete={({ id }) => {
                  afterDelete.deleting(id);
                  send({ type: "DeleteSideChat", chatId: id }).catch(
                    afterDelete.kept,
                  );
                }}
              />
            ))}
          </ul>
        )}
      </div>
      <div className="shrink-0 border-t border-edge px-4 py-2.5">
        <ModelPicker />
      </div>
    </>
  );
}
