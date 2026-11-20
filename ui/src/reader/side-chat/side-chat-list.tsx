import { useEffect, useRef } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { ModelPicker } from "@/model-picker/model-picker";
import { selectSideChats } from "@/store";

/**
 * The course's side chats, newest first, each by its passage (quoted) or
 * its first question and the section it was asked in; choosing one opens
 * it. The model in use at the foot. `focusChatId` names the chat just left,
 * whose entry takes focus.
 */
export function SideChatList({
  courseId,
  focusChatId,
}: {
  courseId: string;
  focusChatId: string | null;
}) {
  const { send } = useAtlasStore();
  const chats = useAtlas((state) => selectSideChats(state, courseId));
  const focused = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    send({ type: "ListSideChats", courseId }).catch(failureInStore);
  }, [send, courseId]);

  const listed = chats !== null;
  useEffect(() => {
    if (listed) focused.current?.focus();
  }, [listed]);

  return (
    <>
      <div className="min-h-0 flex-1 overflow-y-auto px-2 py-3">
        {chats === null ? (
          <p role="status" className="px-2 text-13 text-muted-foreground">
            Loading side chats…
          </p>
        ) : chats.length === 0 ? (
          <p className="px-2 text-13 text-muted-foreground">
            No side chats yet: ask about a selected passage, or start one with
            New side chat.
          </p>
        ) : (
          <ul aria-label="Side chats" className="flex flex-col gap-0.5">
            {chats.map((chat) => (
              <li key={chat.id}>
                <button
                  type="button"
                  ref={chat.id === focusChatId ? focused : undefined}
                  onClick={() =>
                    send({ type: "OpenSideChat", chatId: chat.id }).catch(
                      failureInStore,
                    )
                  }
                  className="flex w-full flex-col gap-0.5 rounded-md px-2 py-2 text-left transition-colors outline-none hover:bg-wash focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid active:bg-wash-strong motion-reduce:transition-none"
                >
                  <span className="line-clamp-2 text-13 text-body">
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
              </li>
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
